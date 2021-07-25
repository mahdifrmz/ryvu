use lex::lex;
pub use lex::LexerError;
use parse::parse;
pub use parse::ParserError;
use module::Module;
use translate::translate;

#[macro_use]
mod lex;
#[macro_use]
mod translate;
mod parse;

pub struct CompilationResult {
    pub module:Option<Module>,
    pub success:bool,
    pub perrors:Vec<ParserError>,
    pub lerrors:Vec<LexerError>,
    pub input_ids:Option<Vec<String>>,
    pub output_ids:Option<Vec<String>>
}

pub fn compile(source:&str,gen_ids:bool,io_min:bool)-> CompilationResult {
    let (tokens,lexer_error) = lex(source);
    let (connections,parser_error) = parse(tokens,io_min);
    if lexer_error.len() > 0 || parser_error.len() > 0 {
        CompilationResult{
            module:None,
            success:false,
            perrors:parser_error,
            lerrors:lexer_error,
            input_ids:None,
            output_ids:None
        }
    }
    else{
        let tr = translate(connections,gen_ids);
        let (input_ids,output_ids) = match tr.identifiers {
            Some((ins ,outs))=>(Some(ins),Some(outs)),
            None=>(None,None)
        };
        CompilationResult{
            module:Some(tr.module),
            success:true,
            perrors:vec![],
            lerrors:vec![],
            input_ids,
            output_ids
        }
    }
}