use crate::cli::App;

mod cli;
mod error;
mod parse;
mod perm;

use regex_syntax::Parser;

fn main() {
    let m = App().get_matches();
    let mut parser = Parser::new();

    if let Some(pattern) = m.get_one::<String>("pattern") {
        println!("Pattern is {:#?}", pattern);
        if let Some(ast) = parser.parse(&pattern).ok() {
            println!("Ast is {:#?}", ast);
        }
    }
}
