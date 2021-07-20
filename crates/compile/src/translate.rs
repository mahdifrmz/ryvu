use std::collections::HashMap;
use module::{Module, ModuleBuilder};
use crate::lex::{SourcePosition, Token, TokenKind};

#[derive(Default)]
struct Translator {
    state:TranslatorState,
    indexer:Indexer,
    connections:Vec<Connection>,
    once:String,
    is_charge:bool,
    is_port:bool,
    errors:Vec<TranslatorError>
}

#[derive(Debug,PartialEq,Eq,Clone, Copy)]
enum IdentKind {
    Node,
    InPort,
    OutPort
}

#[derive(Debug,PartialEq, Eq)]
enum TranslatorError {
    UnexpectedToken(SourcePosition),
    UnexpectedEnd,
    InconstIdent(String,IdentKind,IdentKind)
}

#[derive(PartialEq, Eq)]
enum TranslatorState {
    Statement,
    Operator,
    Identifier,
    PortIdent,
    PortStmt,
    Terminate,
    Error
}

#[derive(Default)]
struct Indexer{
    map:HashMap<String,(usize,IdentKind)>
}

struct Connection {
    from: Identifier,
    to: Identifier,
    is_charge:bool
}

impl Connection {
    fn new(from: Identifier, to: Identifier, is_charge: bool) -> Connection {
        Connection { from, to, is_charge }
    }
}

struct Identifier {
    name:String,
    is_port:bool,
}

impl Identifier {
    fn new(name:String,is_port:bool)->Identifier{
        Identifier{name,is_port}
    }
}

fn translate(tokens:Vec<Token>)->(Module,Vec<TranslatorError>){
    let mut translator = Translator::default();
    translator.translate(tokens)
}

impl Translator {

    fn translate(&mut self,tokens:Vec<Token>)->(Module,Vec<TranslatorError>) {
        self.extract_tuples(tokens);
        (self.build(),std::mem::replace(&mut self.errors, vec![]))
    }

    fn build(&mut self)->Module {
        let mut builder = ModuleBuilder::default();
        for con in self.connections.iter() {

            let from_kind = if con.from.is_port {
                IdentKind::InPort
            }
            else{
                IdentKind::Node
            };
            let from = self.indexer.index(con.from.name.clone(),from_kind);

            let to_kind = if con.to.is_port {
                IdentKind::OutPort
            }
            else{
                IdentKind::Node
            };
            let to = self.indexer.index(con.to.name.clone(),to_kind);

            let mut flag = false;
            if let Some(kind) = from.1 {
                flag = true;
                self.errors.push(TranslatorError::InconstIdent(con.from.name.clone(),kind,from_kind));
            }
            if let Some(kind) = to.1 {
                flag = true;
                self.errors.push(TranslatorError::InconstIdent(con.to.name.clone(),kind,to_kind));
            }

            if !flag {
                builder.connect(from.0, to.0, con.is_charge);
                if from.2 && from_kind == IdentKind::InPort {
                    builder.input(from.0);
                }
                if to.2 && to_kind == IdentKind::OutPort {
                    builder.output(to.0);
                }
            }
        }
        builder.build()
    }

    fn extract_tuples(&mut self,tokens:Vec<Token>) {
        for token in tokens.iter() {
            if  self.state == TranslatorState::Error && 
                token.kind() != TokenKind::Semicolon && 
                token.kind() != TokenKind::EndLine 
            
            {
                continue;
            }

            self.handle_token(token);
        }
        self.finalize();
    }
    fn handle_token(&mut self,token:&Token){
        
        match token.kind() {
            TokenKind::Identifier=>{
                self.handle_ident(token);
            },
            TokenKind::Charge | TokenKind::Block=>{
                self.handle_operator(token);
            },
            TokenKind::Semicolon=>{
                self.handle_semicolon(token);
            },
            TokenKind::EndLine=>{
                self.handle_endline();
            },
            TokenKind::Port=>{
                self.handle_port(token);
            },
            TokenKind::Space=>{
                self.handle_space(token);
            }
        }
    }
    fn finalize(&mut self) {
        if self.state == TranslatorState::Operator || self.state == TranslatorState::Identifier {
            self.unexpected_end();
        }
    }
    fn handle_ident(&mut self,token:&Token){
        match self.state {
            TranslatorState::Statement => {
                self.once = token.text().to_owned();
                self.state = TranslatorState::Operator;
                self.is_port = false;
            },
            TranslatorState::PortStmt => {
                self.once = token.text().to_owned();
                self.state = TranslatorState::Operator;
                self.is_port = true;
            },
            TranslatorState::Identifier => {
                self.connect(token.text(),false);
                self.once = token.text().to_owned();
                self.is_port = false;
                self.state = TranslatorState::Terminate;
            },
            TranslatorState::PortIdent => {
                self.connect(token.text(),true);
                self.once = token.text().to_owned();
                self.is_port = true;
                self.state = TranslatorState::Terminate;
            },
            _ =>{
                self.unexpected_error(token);
                self.state = TranslatorState::Error;
            }
        }
    }
    fn handle_space(&mut self,token:&Token){
        match self.state  {
            TranslatorState::PortIdent | TranslatorState::PortStmt => {
                self.unexpected_error(token);
                self.state = TranslatorState::Error;
            }
            _=>{
                // nothing
            }
        }
    }
    fn handle_semicolon(&mut self,token:&Token){
        match self.state {
            TranslatorState::Terminate | TranslatorState::Error => {
                self.once = String::new();
                self.state = TranslatorState::Statement;
            },
            _ =>{
                self.unexpected_error(token);
                self.state = TranslatorState::Error;
            }
        }
    }
    fn handle_operator(&mut self,token:&Token){
        match self.state {
            TranslatorState::Terminate | TranslatorState::Operator=> {
                if token.kind() == TokenKind::Charge {
                    self.is_charge = true;
                }
                else{
                    self.is_charge = false;
                }
                self.state = TranslatorState::Identifier;
            },
            _ =>{
                self.unexpected_error(token);
                self.state = TranslatorState::Error;
            }
        }
    }
    fn handle_port(&mut self,token:&Token){
        match self.state {
            TranslatorState::Identifier => {
                self.state = TranslatorState::PortIdent;
            },
            TranslatorState::Statement => {
                self.state = TranslatorState::PortStmt;
            }
            _ =>{
                self.unexpected_error(token);
                self.state = TranslatorState::Error;
            }
        }
    }
    fn handle_endline(&mut self){
        match self.state {
            TranslatorState::Terminate | TranslatorState::Error => {
                self.once = String::new();
                self.state = TranslatorState::Statement;
            },
            _ => {
                // nothing
            }
        }
    }
    fn connect(&mut self,token_text:&str,port:bool){
        let from = Identifier::new(self.once.clone(),self.is_port);
        let to = Identifier::new(token_text.to_owned(),port);
        self.connections.push(Connection::new(from, to, self.is_charge));
    }
    fn unexpected_error(&mut self,token:&Token){
        self.errors.push(TranslatorError::UnexpectedToken(token.position()))
    }
    fn unexpected_end(&mut self){
        self.errors.push(TranslatorError::UnexpectedEnd);
    }
}

impl Default for TranslatorState {
    fn default() -> Self {
        TranslatorState::Statement
    }
}

impl Indexer {
    fn index(&mut self,ident:String,kind:IdentKind)->(usize,Option<IdentKind>,bool) {
        match self.map.get(&ident) {
            Some(index)=> {
                if index.1 == kind {
                    (index.0,None,false)
                }
                else{
                    (index.0,Some(index.1),false)
                }
            },
            None=>{
                self.map.insert(ident, (self.map.len(),kind));
                (self.map.len()-1,None,true)
            }
        }
    }
}


#[cfg(test)]
mod test {

    use module::ModuleBuilder;
    use crate::{lex::SourcePosition, translate::{IdentKind, Module, Token, TranslatorError, translate}};

    fn module_test_case(tokens:Vec<Token>,module:Module){
        let compiled_module = translate(tokens).0;
        assert_eq!(compiled_module,module);
    }
    fn error_test_case(tokens:Vec<Token>,errors:Vec<TranslatorError>){
        assert_eq!(translate(tokens).1,errors);
    }

    #[test]
    fn no_tokens(){
        module_test_case(vec![], Module::default())
    }

    #[test]
    fn ignores_spaces(){
        module_test_case(vec![
            token!(Space,"   ",0,0),
            token!(EndLine,"\n",0,3),
            token!(Space,"    ",1,0)
        ], Module::default())
    }

    #[test]
    fn single_charge(){
        let mut module = ModuleBuilder::default();
        module.charge(0, 1);
        module_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Charge,">",0,1),
            token!(Identifier,"b",0,2)
        ], module.build())
    }

    #[test]
    fn single_charge_with_space(){
        let mut module = ModuleBuilder::default();
        module.charge(0, 1);
        module_test_case(vec![
            token!(Space,"    ",0,0),
            token!(Identifier,"a",0,4),
            token!(Space,"   ",0,5),
            token!(Charge,">",0,8),
            token!(Space,"  ",0,9),
            token!(Identifier,"b",0,11),
            token!(Space," ",0,12)
        ], module.build())
    }

    #[test]
    fn single_charge_same_node(){
        let mut module = ModuleBuilder::default();
        module.charge(0, 0);
        module_test_case(vec![
            token!(Space,"    ",0,0),
            token!(Identifier,"a",0,4),
            token!(Space,"   ",0,5),
            token!(Charge,">",0,8),
            token!(Space,"  ",0,9),
            token!(Identifier,"a",0,11),
            token!(Space," ",0,12)
        ], module.build())
    }


    #[test]
    fn chained_statements(){
        let mut module = ModuleBuilder::default();
        module.block(0, 1);
        module.charge(1, 2);
        module_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Space,"   ",0,1),
            token!(Block,".",0,4),
            token!(Space,"  ",0,5),
            token!(Identifier,"b",0,7),
            token!(Charge,">",0,8),
            token!(Identifier,"c",0,9),
        ], module.build())
    }

    #[test]
    fn chained_statements_reoccurring_idents(){
        let mut module = ModuleBuilder::default();
        module.block(0, 1);
        module.charge(1, 0);
        module_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Space,"   ",0,1),
            token!(Block,".",0,4),
            token!(Space,"  ",0,5),
            token!(Identifier,"b",0,7),
            token!(Charge,">",0,8),
            token!(Identifier,"a",0,9),
        ], module.build())
    }


    #[test]
    fn semincolon_statement_seperation(){
        let mut module = ModuleBuilder::default();
        module.block(0, 1);
        module.charge(1, 2);
        module.charge(0, 3);
        module_test_case(vec![
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
        ], module.build())
    }

    #[test]
    fn passes_on_sequential_identifiers(){
        let mut module = ModuleBuilder::default();
        module.block(0, 1);
        module.charge(0, 0);
        module_test_case(vec![
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
        ], module.build())
    }

    #[test]
    fn error_on_sequential_identifiers(){
        error_test_case(vec![
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
            TranslatorError::UnexpectedToken(SourcePosition::new(0,12))
        ])
    }

    #[test]
    fn ignores_endline_in_statements(){
        let mut module = ModuleBuilder::default();
        module.block(0, 1);
        module_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(EndLine,"\n",0,1),
            token!(Block,".",0,2),
            token!(EndLine,"\n",0,3),
            token!(Identifier,"b",0,4)
        ], module.build())
    }

    #[test]
    fn endline_terminates_statement(){
        let mut module = ModuleBuilder::default();
        module.block(0, 1);
        module.charge(0, 2);
        module_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(EndLine,"\n",0,1),
            token!(Block,".",0,2),
            token!(EndLine,"\n",0,3),
            token!(Identifier,"b",0,4),
            token!(EndLine,"\n",0,5),
            token!(Identifier,"a",1,0),
            token!(Charge,">",1,1),
            token!(Identifier,"c",1,2),
        ], module.build())
    }

    #[test]
    fn endline_recovers_after_error(){
        let mut module = ModuleBuilder::default();
        module.charge(0, 1);
        module_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Block,".",0,1),
            token!(Block,".",0,2),
            token!(EndLine,"\n",0,3),
            token!(Identifier,"a",1,0),
            token!(Charge,">",1,1),
            token!(Identifier,"c",1,2),
        ], module.build())
    }

    #[test]
    fn error_on_unexpected_end(){
        error_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Block,".",0,1)
        ], vec![
            TranslatorError::UnexpectedEnd
        ])
    }

    #[test]
    fn input_ports(){
        let mut module = ModuleBuilder::default();
        module.charge(0, 1);
        module.input(0);
        module_test_case(vec![
            token!(Port,"$",0,0),
            token!(Identifier,"a",0,1),
            token!(Charge,">",0,2),
            token!(Space,"  ",0,3),
            token!(Identifier,"b",0,5)
        ], module.build())
    }
    #[test]
    fn error_port_notfollewedby_ident(){
        error_test_case(vec![
            token!(Port,"$",0,0),
            token!(Space," ",0,1),
            token!(Identifier,"a",0,2),
            token!(Charge,">",0,3),
            token!(Space,"  ",0,4),
            token!(Identifier,"b",0,6)
        ], vec![
            TranslatorError::UnexpectedToken(SourcePosition::new(0,1))
        ])
    }

    #[test]
    fn output_ports(){
        let mut module = ModuleBuilder::default();
        module.charge(0, 1);
        module.output(1);
        module_test_case(vec![
            token!(Identifier,"a",0,0),
            token!(Charge,">",0,1),
            token!(Port,"$",0,2),
            token!(Identifier,"b",0,3)
        ], module.build())
    }

    #[test]
    fn error_inconsistant_ident_type(){
        error_test_case(vec![
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
            TranslatorError::InconstIdent("b".to_owned(),IdentKind::OutPort,IdentKind::InPort),
            TranslatorError::InconstIdent("a".to_owned(),IdentKind::Node,IdentKind::OutPort),
            TranslatorError::InconstIdent("a".to_owned(),IdentKind::Node,IdentKind::InPort)
        ])
    }
}