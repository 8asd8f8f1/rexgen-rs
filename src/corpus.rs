use regex_syntax::Parser;
use regex_syntax::hir::Hir;
use regex_syntax::hir::HirKind;

use crate::error::Error;
use crate::error::Result;

pub(crate) struct Corpus {
    ast: Hir,
}

impl Corpus {
    pub(crate) fn new(pattern: &str) -> Result<Self> {
        let ast = Parser::new()
            .parse(pattern)
            .map_err(|_| Error::Message("Invalid Pattern".to_string()))?;

        Ok(Self { ast })
    }

    pub(crate) fn generate(&self) -> Result<()> {
        match self.ast.kind() {
            HirKind::Empty => println!("empty"),
            HirKind::Literal(literal) => println!("literal: {:#?}", literal),
            HirKind::Class(class) => println!("class: {:#?}", class),
            HirKind::Repetition(rep) => println!("rep: {:#?}", rep),
            HirKind::Capture(capture) => println!("capture: {:#?}", capture),
            HirKind::Concat(concat) => println!("concat: {:#?}", concat),
            _ => println!("none"),
        }
        Ok(())
    }
}
