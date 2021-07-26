use std::{collections::HashMap};

use crate::{lex::{SourcePosition, Token, TokenKind}, translate::{Connection, IdentKind, Identifier}};

#[derive(Default)]
struct Parser{
    inverter:Inverter,
    connections:Vec<Connection>,
    buffer:ConBuf,
    errors:Vec<ParserError>,
    id_map:IdMap
}

#[derive(Default)]
struct Inverter{
    tokens:Vec<Token>,
    index:usize,
    state:InverterState,
    stack:Vec<Token>
}

#[derive(PartialEq, Eq)]
enum InverterState {
    Normal,
    WasPort,
    WasIdent,
    WasEndl(Token)
}

impl Default for InverterState {
    fn default() -> Self {
        InverterState::Normal
    }
}

impl Inverter {
    fn new(tokens:Vec<Token>) -> Inverter {
        Inverter{
            tokens,
            state:InverterState::Normal,
            index:0,
            stack:vec![]
        }
    }
    fn consume_end(&mut self){
        while let Some(token) = self.stack.last().cloned() {
            self.stack.pop();
            if token.kind() == TokenKind::Semicolon || token.kind() == TokenKind::EndLine {
                return;
            }
        }
        loop {
            if self.tokens.len() <= self.index {
                break;
            }
            let t = self.tokens[self.index].kind();
            if t == TokenKind::Semicolon || t == TokenKind::EndLine {
                break;
            }
            else {
                self.index += 1;
            }
        }
        self.state = InverterState::Normal;
    }
    fn expect(&mut self)-> Option<Token> {
        let t = self.peek();
        self.stack.pop();
        t
    }
    fn peek(&mut self) -> Option<Token> {
        while self.index < self.tokens.len() && self.stack.is_empty() {
            self.get();
        }
        return self.stack.last().cloned()
    }
    fn get(&mut self) {
        let token = self.tokens[self.index].clone();
        self.index += 1;
        match token.kind() {
            TokenKind::Charge | TokenKind::Block | TokenKind::Comma | TokenKind::Semicolon => {
                self.state = InverterState::Normal;
                self.stack.push(token);
            },
            TokenKind::Space => {
                if self.state == InverterState::WasPort {
                    self.stack.push(token);
                    self.state = InverterState::Normal;
                }
            },
            TokenKind::Port => {
                self.state = InverterState::WasPort;
                self.stack.push(token);
            }
            TokenKind::Identifier => {
                self.stack.push(token);
                match &self.state {
                    InverterState::WasEndl(endl) => self.stack.push(endl.clone()),
                    _=>{}
                }
                self.state = InverterState::WasIdent;
            },
            TokenKind::EndLine => {
                if self.state == InverterState::WasIdent {
                    self.state = InverterState::WasEndl(token);
                }
            }
        }
    }
}

#[derive(Default)]
struct ConBuf {
    from:Vec<IdPair>,
    to:Vec<IdPair>,
    is_charge:bool
}

#[derive(Clone)]
struct IdPair(String,bool);

#[derive(PartialEq, Eq,Clone, Copy)]
enum OperatorKind {
    Charge,
    Block,
    Comma
}

type IdMap = HashMap<String,IdentKind>;

#[derive(Debug,PartialEq, Eq)]
pub enum ParserError {
    UnexpectedToken(SourcePosition),
    UnexpectedEnd,
    IOMin,
    InconstIdKind(String,IdentKind,IdentKind)
}

pub fn parse(tokens:Vec<Token>,io_min:bool)->(Vec<Connection>,Vec<ParserError>) {
    Parser::default().parse(tokens,io_min)
}

impl Parser {

    fn parse(&mut self,tokens:Vec<Token>,io_min:bool) -> (Vec<Connection>,Vec<ParserError>) {
        self.inverter = Inverter::new(tokens);
        while let Some(_) = self.peek_token() {
            if self.expect_source() == None {
                self.inverter.consume_end();
                self.clear_buffer();
            }
        }
        self.finalize(io_min)
    }

    fn expect_source(&mut self)->Option<()>{
        self.expect_statement()?;
        while let Some(_) = self.peek(&[TokenKind::Semicolon,TokenKind::EndLine]) {
            self.consume_token();
            self.expect_statement()?;
        }
        Some(())
    }

    fn expect_statement(&mut self)->Option<()>{
        while let Some(_) = self.peek(&[TokenKind::Semicolon,TokenKind::EndLine]) {
            self.consume_token();
        }
        if let Some(_) = self.peek(&[TokenKind::Identifier,TokenKind::Port]) {
            self.expect_batch(OperatorKind::default())?;
            self.expect_operation()?;
            while let Some(_) = self.peek(&[TokenKind::Charge,TokenKind::Block]) {    
                self.expect_operation()?;
            }
            self.connect();
            self.clear_buffer();   
        }
        Some(())
    }

    fn expect_operation(&mut self) -> Option<()> {
        let opr = self.expect(&[TokenKind::Charge,TokenKind::Block])?;
        self.expect_batch(match opr.kind() {
            TokenKind::Charge => OperatorKind::Charge,
            TokenKind::Block => OperatorKind::Block,
            _=>OperatorKind::default()
        })
    }

    fn expect_batch(&mut self,operator_kind:OperatorKind)->Option<()>{
        let mut id = self.expect_id()?;
        self.new_ident(id.0.as_str(), id.1, operator_kind);
        while let Some(_) = self.peek(&[TokenKind::Comma]) {
            self.consume_token();
            id = self.expect_id()?;
            self.new_ident(id.0.as_str(), id.1, OperatorKind::Comma);
        }
        Some(())
    }

    fn expect_id(&mut self)->Option<IdPair> {
        let t1 = self.expect_token()?;
        match t1.kind() {
            TokenKind::Identifier=>{ 
                Some(IdPair(t1.text().to_owned(),false))
            },
            TokenKind::Port=>{
                let t2 = self.expect(&[TokenKind::Identifier])?;
                Some(IdPair(t2.text().to_owned(),true))
            },
            _=>{
                self.err_unexpected_token(&t1);
                None
            }
        }
    }

    fn consume_token(&mut self){
        self.inverter.expect();
    }

    fn expect_token(&mut self) -> Option<Token>{
        match self.inverter.expect() {
            None => {
                self.err_unexpected_end();
                None
            },
            Some(token)=> Some(token)
        }
    }

    fn peek_token(&mut self) -> Option<Token>{
        self.inverter.peek()
    }

    fn expect(&mut self,kinds:&[TokenKind]) -> Option<Token>{
        let t = self.expect_token()?;
        if kinds.contains(&t.kind()) {
            Some(t)   
        }
        else{
            self.err_unexpected_token(&t);
            None
        }
    }

    fn peek(&mut self,kinds:&[TokenKind]) -> Option<Token>{
        let token = self.peek_token()?;
        if kinds.contains(&token.kind()) {
            Some(token)
        }
        else {
            None
        }
    }

    fn finalize(&mut self,io_min:bool) -> (Vec<Connection>,Vec<ParserError>){
        if io_min && self.errors.len() == 0 && !self.check_io_min() {
            self.errors.push(ParserError::IOMin);
        }
        (
            std::mem::replace(&mut self.connections, vec![]),
            std::mem::replace(&mut self.errors, vec![])
        )
    }

    fn check_io_min(&self)->bool{
        let mut iflag = false;
        let mut oflag = false;
        for k in self.id_map.values() {
            if *k == IdentKind::InPort {
                iflag = true;
                if oflag {
                    return true;
                }
            }
            else if *k == IdentKind::OutPort {
                oflag = true;
                if iflag {
                    return true;
                }
            }
        }
        false
    }

    fn new_ident(&mut self,token_text:&str,port:bool,operator_kind:OperatorKind){
        if operator_kind == OperatorKind::Comma {
            self.buffer.to.push(IdPair(token_text.to_owned(),port));
        }
        else{
            if self.buffer.from.len() > 0 {
                self.connect();
            }
            self.buffer.from = std::mem::take(&mut self.buffer.to);
            self.buffer.is_charge = operator_kind == OperatorKind::Charge;
            self.buffer.to.push(IdPair(token_text.to_owned(),port));
        }
    }

    fn connect(&mut self){
        for from in 0..self.buffer.from.len() {
            for to in 0..self.buffer.to.len() {
                self.connect_pair(self.buffer.from[from].clone(),self.buffer.to[to].clone());
            }    
        }
    }

    fn clear_buffer(&mut self){
        self.buffer.from.clear();
        self.buffer.to.clear();
    }

    fn connect_pair(&mut self,from:IdPair,to:IdPair){
        let from_kind = self.get_ident_kind(from.1, true);
        let to_kind = self.get_ident_kind(to.1, false);
        self.check_ident_kind(&from.0, from_kind);
        self.check_ident_kind(&to.0, to_kind);
        let from = Identifier::new(from.0, from_kind);
        let to = Identifier::new(to.0, to_kind);
        self.connections.push(Connection::new(from, to, self.buffer.is_charge));
    }

    fn get_ident_kind(&self,is_port:bool,is_from:bool) -> IdentKind {
        if is_port {
            if is_from {
                IdentKind::InPort
            }
            else{
                IdentKind::OutPort
            }
        }
        else{
            IdentKind::Node
        }
    }

    fn check_ident_kind(&mut self,name:&String,kind:IdentKind){
        match self.id_map.get(name).copied() {
            Some(act_kind) => {
                if kind != act_kind {
                    self.err_inconst_ident_kind(name.clone(),kind,act_kind);
                }
            },
            None => {
                self.id_map.insert(name.clone(), kind);
            }
        }
    }

    fn err_unexpected_token(&mut self,token:&Token){
        self.errors.push(ParserError::UnexpectedToken(token.position()))
    }

    fn err_unexpected_end(&mut self){
        self.errors.push(ParserError::UnexpectedEnd);
    }

    fn err_inconst_ident_kind(&mut self,name:String,kind:IdentKind,act_kind:IdentKind){
        self.errors.push(ParserError::InconstIdKind(name,kind,act_kind));
    }
}

impl Default for OperatorKind {
    fn default() -> Self {
        OperatorKind::Charge
    }
}

#[cfg(test)]
mod test_inverter{
    use crate::{lex::Token, parse::Inverter};

    fn invertor_test_case(tokens:Vec<Token>,inverted:Vec<Token>){
        let mut inv = Inverter::new(tokens);
        let mut gen = vec![];
        while let Some(token) = inv.expect() {
            gen.push(token);
        }
        assert_eq!(gen,inverted);
    }

    #[test]
    fn no_token(){
        invertor_test_case(vec![], vec![]);
    }

    #[test]
    fn simple_tokens(){
        invertor_test_case(vec![
            token!(Charge,">"),
            token!(Block,"."),
            token!(Comma,","),
            token!(Semicolon,";")
        ], vec![
            token!(Charge,">"),
            token!(Block,"."),
            token!(Comma,","),
            token!(Semicolon,";")
        ]);
    }



    #[test]
    fn space(){
        invertor_test_case(vec![
            token!(Space,"   "),
            token!(Charge,">"),
            token!(Block,"."),
            token!(Comma,","),
            token!(Space,"     "),
            token!(Semicolon,";"),
            token!(Space,"     ")
        ], vec![
            token!(Charge,">"),
            token!(Block,"."),
            token!(Comma,","),
            token!(Semicolon,";")
        ]);
    }

    #[test]
    fn identifiers(){
        invertor_test_case(vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Comma,","),
        ], vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Comma,","),
        ]);
    }

    #[test]
    fn identifier_followed_by_endl(){
        invertor_test_case(vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(EndLine,"\n"),
            token!(Comma,","),
        ], vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Comma,","),
        ]);
    }

    #[test]
    fn endl_followed_by_identifier(){
        invertor_test_case(vec![
            token!(Block,"."),
            token!(EndLine,"\n"),
            token!(Identifier,"s"),
            token!(Comma,","),
        ], vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Comma,","),
        ]);
    }

    #[test]
    fn endl_surrounded_by_identifier(){
        invertor_test_case(vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(EndLine,"\n"),
            token!(Identifier,"s"),
            token!(Comma,","),
        ], vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(EndLine,"\n"),
            token!(Identifier,"s"),
            token!(Comma,",")
        ]);
    }

    #[test]
    fn endl_surrounded_by_identifiers_and_spaces(){
        invertor_test_case(vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Space," "),
            token!(Space,"    "),
            token!(EndLine,"\n"),
            token!(Space,"   "),
            token!(Identifier,"s"),
            token!(Comma,","),
        ], vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(EndLine,"\n"),
            token!(Identifier,"s"),
            token!(Comma,","),
        ]);
    }

    #[test]
    fn port(){
        invertor_test_case(vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Space," "),
            token!(Port,"$"),
            token!(Identifier,"s"),
            token!(Comma,","),
        ], vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Port,"$"),
            token!(Identifier,"s"),
            token!(Comma,","),
        ]);
    }

    #[test]
    fn space_after_port(){
        invertor_test_case(vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Space," "),
            token!(Port,"$"),
            token!(Space,"    "),
            token!(Identifier,"s"),
            token!(Comma,","),
        ], vec![
            token!(Block,"."),
            token!(Identifier,"s"),
            token!(Port,"$"),
            token!(Space,"    "),
            token!(Identifier,"s"),
            token!(Comma,","),
        ]);
    }
}



#[cfg(test)]
mod test_parser {
    use crate::{lex::{SourcePosition,Token}, translate::{Connection,IdentKind}, parse::{parse,ParserError}};

    fn parser_test_case(tokens:Vec<Token>,connections:Vec<Connection>){
        let pr = parse(tokens,false);
        assert_eq!(pr.1,vec![]);
        assert_eq!(pr.0,connections);
    }

    fn parse_error_test_case(tokens:Vec<Token>,errors:Vec<ParserError>){
        let generated_errors = parse(tokens,false).1;
        assert_eq!(generated_errors,errors);
    }

    fn parse_test_case_force_output(tokens:Vec<Token>,connections:Vec<Connection>){
        let generated_errors = parse(tokens,false).0;
        assert_eq!(generated_errors,connections);
    }

    fn parse_error_test_case_io_min(tokens:Vec<Token>,errors:Vec<ParserError>){
        let generated_errors = parse(tokens,true).1;
        assert_eq!(generated_errors,errors);
    }

    #[test]
    fn empty(){
        parser_test_case(vec![], vec![])
    }

    #[test]
    fn no_tokens(){
        parser_test_case(vec![], vec![])
    }

    #[test]
    fn ignores_spaces(){
        parser_test_case(vec![
            token!(Space,"   ",0,0),
            token!(EndLine,"\n",0,3),
            token!(Space,"    ",1,0)
        ], vec![])
    }

    #[test]
    fn single_charge(){
        parser_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Charge,">",0,1),
            token!(Identifier,"b",0,2)
        ], vec![
            connection!(a > b)
        ])
    }

    #[test]
    fn single_charge_with_space(){
        parser_test_case(vec![
            token!(Space,"    ",0,0),
            token!(Identifier,"a",0,4),
            token!(Space,"   ",0,5),
            token!(Charge,">",0,8),
            token!(Space,"  ",0,9),
            token!(Identifier,"b",0,11),
            token!(Space," ",0,12)
        ], vec![
            connection!(a > b)
        ])
    }

    #[test]
    fn single_charge_same_node(){
        parser_test_case(vec![
            token!(Space,"    ",0,0),
            token!(Identifier,"a",0,4),
            token!(Space,"   ",0,5),
            token!(Charge,">",0,8),
            token!(Space,"  ",0,9),
            token!(Identifier,"a",0,11),
            token!(Space," ",0,12)
        ], vec![
            connection!(a > a)
        ])
    }

    #[test]
    fn chained_statements(){
        parser_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Space,"   ",0,1),
            token!(Block,".",0,4),
            token!(Space,"  ",0,5),
            token!(Identifier,"b",0,7),
            token!(Charge,">",0,8),
            token!(Identifier,"c",0,9),
        ], vec![
            connection!(a . b),
            connection!(b > c)
        ])
    }

    #[test]
    fn chained_statements_reoccurring_idents(){
        parser_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Space,"   ",0,1),
            token!(Block,".",0,4),
            token!(Space,"  ",0,5),
            token!(Identifier,"b",0,7),
            token!(Charge,">",0,8),
            token!(Identifier,"a",0,9),
        ], vec![
            connection!(a . b),
            connection!(b > a)
        ])
    }

    #[test]
    fn semicolon_statement_seperation(){
        parser_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Space,"   ",0,1),
            token!(Block,".",0,4),
            token!(Space,"  ",0,5),
            token!(Identifier,"b",0,7),
            token!(Charge,">",0,8),
            token!(Identifier,"c",0,9),
            token!(Semicolon,";",0,10),
            token!(Space," ",0,11),
            token!(Identifier,"a",0,12),
            token!(Charge,">",0,13),
            token!(Identifier,"d",0,14),
            token!(Semicolon,";",0,15),
        ], vec![
            connection!(a . b),
            connection!(b > c),
            connection!(a > d)
        ])
    }

    #[test]
    fn passes_on_sequential_identifiers(){
        parse_test_case_force_output(vec![
            token!(Identifier,"a",0,0),
            token!(Space,"   ",0,1),
            token!(Block,".",0,4),
            token!(Space,"  ",0,5),
            token!(Identifier,"b",0,7),
            token!(Semicolon,";",0,8),
            token!(Identifier,"c",0,9),
            token!(Space,"  ",0,10),
            token!(Identifier,"a",0,12),
            token!(Semicolon,";",0,13),
            token!(Identifier,"a",0,14),
            token!(Charge,">",0,15),
            token!(Identifier,"a",0,16),
        ], vec![
            connection!(a . b),
            connection!(a > a),
        ])
    }

    #[test]
    fn error_on_sequential_identifiers(){
        parse_error_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Space,"   ",0,1),
            token!(Block,".",0,4),
            token!(Space,"  ",0,5),
            token!(Identifier,"b",0,7),
            token!(Semicolon,";",0,8),
            token!(Identifier,"c",0,9),
            token!(Space,"  ",0,10),
            token!(Identifier,"a",0,12),
            token!(Semicolon,";",0,13),
            token!(Identifier,"a",0,14),
            token!(Charge,">",0,15),
            token!(Identifier,"a",0,16),
        ], vec![
            ParserError::UnexpectedToken(SourcePosition::new(0,12))
        ])
    }

    #[test]
    fn ignores_endline_in_statements(){
        parser_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(EndLine,"\n",0,1),
            token!(Block,".",0,2),
            token!(EndLine,"\n",0,3),
            token!(Identifier,"b",0,4)
        ], vec![
            connection!(a . b)
        ])
    }

    #[test]
    fn endline_terminates_statement(){
        parser_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(EndLine,"\n",0,1),
            token!(Block,".",0,2),
            token!(EndLine,"\n",0,3),
            token!(Identifier,"b",0,4),
            token!(EndLine,"\n",0,5),
            token!(Identifier,"a",1,0),
            token!(Charge,">",1,1),
            token!(Identifier,"c",1,2),
        ], vec![
            connection!(a . b),
            connection!(a > c)
        ])
    }

    #[test]
    fn endline_recovers_after_error(){
        parse_test_case_force_output(vec![
            token!(Identifier,"a",0,0),
            token!(Block,".",0,1),
            token!(Block,".",0,2),
            token!(EndLine,"\n",0,3),
            token!(Identifier,"a",1,0),
            token!(Charge,">",1,1),
            token!(Identifier,"c",1,2),
        ], vec![
            connection!(a > c)
        ])
    }

    #[test]
    fn error_on_unexpected_end(){
        parse_error_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Block,".",0,1)
        ], vec![
            ParserError::UnexpectedEnd
        ])
    }

    #[test]
    fn input_ports(){
        parser_test_case(vec![
            token!(Port,"$",0,0),
            token!(Identifier,"a",0,1),
            token!(Charge,">",0,2),
            token!(Space,"  ",0,3),
            token!(Identifier,"b",0,5)
        ], vec![
            connection!(!a > b)
        ])
    }

    #[test]
    fn error_port_notfollewedby_ident(){
        parse_error_test_case(vec![
            token!(Port,"$",0,0),
            token!(Space," ",0,1),
            token!(Identifier,"a",0,2),
            token!(Charge,">",0,3),
            token!(Space,"  ",0,4),
            token!(Identifier,"b",0,6)
        ], vec![
            ParserError::UnexpectedToken(SourcePosition::new(0,1))
        ])
    }

    #[test]
    fn output_ports(){
        parser_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Charge,">",0,1),
            token!(Port,"$",0,2),
            token!(Identifier,"b",0,3)
        ], vec![
            connection!(a > !b)
        ])
    }


    #[test]
    fn error_inconsistant_ident_type(){
        parse_error_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Charge,">",0,1),
            token!(Port,"$",0,2),
            token!(Identifier,"b",0,3),
            token!(Semicolon,";",0,4),

            token!(Port,"$",0,5),
            token!(Identifier,"b",0,6),
            token!(Charge,">",0,7),
            token!(Port,"$",0,8),
            token!(Identifier,"a",0,9),
            token!(Semicolon,";",0,10),

            token!(Port,"$",0,11),
            token!(Identifier,"a",0,12),
            token!(Charge,">",0,13),
            token!(Identifier,"c",0,14),
        ], vec![
            ParserError::InconstIdKind("b".to_owned(),IdentKind::InPort,IdentKind::OutPort),
            ParserError::InconstIdKind("a".to_owned(),IdentKind::OutPort,IdentKind::Node),
            ParserError::InconstIdKind("a".to_owned(),IdentKind::InPort,IdentKind::Node)
        ])
    }

    #[test]
    fn single_connect_node_batching(){
        parser_test_case(vec![
            token!(Identifier,"a"),
            token!(Comma,","),
            token!(Identifier,"b"),
            token!(Comma,","),
            token!(Identifier,"c"),
            token!(Charge,">"),
            token!(Identifier,"d")
        ], vec![
            connection!(a > d),
            connection!(b > d),
            connection!(c > d)
        ])
    }
    
    #[test]
    fn multi_connect_node_batching(){
        parser_test_case(vec![
            token!(Identifier,"a"),
            token!(Charge,">"),
            token!(Identifier,"b1"),
            token!(Comma,","),
            token!(Identifier,"b2"),
            token!(Charge,">"),
            token!(Identifier,"c1"),
            token!(Comma,","),
            token!(Identifier,"c2"),
            token!(Block,"."),
            token!(Identifier,"d")
        ], vec![
            connection!(a > b1),
            connection!(a > b2),
            connection!(b1 > c1),
            connection!(b1 > c2),
            connection!(b2 > c1),
            connection!(b2 > c2),
            connection!(c1 . d),
            connection!(c2 . d)
        ])
    }

    #[test]
    fn port_node_batching(){
        parser_test_case(vec![
            token!(Port,"$"),
            token!(Identifier,"a"),
            token!(Charge,">"),
            token!(Identifier,"b"),
            token!(Comma,","),
            token!(Identifier,"c"),
            token!(Charge,">"),
            token!(Port,"$"),
            token!(Identifier,"d")
        ], vec![
            connection!(!a > b),
            connection!(!a > c),
            connection!(b > !d),
            connection!(c > !d),
        ])
    }

    #[test]
    fn error_inconsistant_ident_type_node_batching(){
        parse_error_test_case(vec![
            token!(Port,"$"),
            token!(Identifier,"a"),
            token!(Charge,">"),
            token!(Port,"$"),
            token!(Identifier,"b"),
            token!(Comma,","),
            token!(Identifier,"c"),
            token!(Charge,">"),
            token!(Port,"$"),
            token!(Identifier,"d")
        ], vec![
            ParserError::InconstIdKind("b".to_owned(),IdentKind::InPort,IdentKind::OutPort)
        ])
    }

    #[test]
    fn error_io_min_violated(){
        parse_error_test_case_io_min(vec![
            token!(Identifier,"a"),
            token!(Charge,">"),
            token!(Identifier,"b"),
            token!(EndLine,"\n"),
            token!(Identifier,"a"),
            token!(Charge,">"),
            token!(Identifier,"c"),
        ], vec![
            ParserError::IOMin
        ])
    }

    #[test]
    fn operater_at_next_line(){
        parser_test_case(vec![
            token!(Identifier,"a"),
            token!(Charge,">"),
            token!(Identifier,"b"),
            token!(Comma,","),
            token!(Identifier,"c"),
            token!(EndLine,"\n"),
            token!(Charge,">"),
            token!(Identifier,"d"),
        ], vec![
            connection!(a > b),
            connection!(a > c),
            connection!(b > d),
            connection!(c > d),
        ])
    }
    #[test]
    fn unexpected_token_after_opr(){
        parse_error_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Charge,">",0,1),
            token!(Block,".",0,2),
        ], vec![
            ParserError::UnexpectedToken(SourcePosition::new(0,2))
        ])
    }

    #[test]
    fn unexpected_token_after_port_sign(){
        parse_error_test_case(vec![
            token!(Port,"$",0,0),
            token!(Charge,">",0,1),
            token!(Block,".",0,2),
        ], vec![
            ParserError::UnexpectedToken(SourcePosition::new(0,1))
        ])
    }

    #[test]
    fn io_min_violation_wihout_basic_errors(){
        parse_error_test_case_io_min(vec![
            token!(Identifier,"a",0,0),
            token!(Charge,">",0,1),
            token!(Block,".",0,2),
        ], vec![
            ParserError::UnexpectedToken(SourcePosition::new(0,2))
        ])
    }
}