use std::default;
use std::rc::Rc;

use crate::error::Result;

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) enum PatternKind {
    #[default]
    None,
    Literal {
        size: u64,
    },
    Fixed {
        size: u64,
    },
    Variable {
        min: u64,
        max: u64,
    },
}

#[derive(Default, Debug, Clone, PartialEq)]
pub(crate) struct Pattern {
    kind: Option<PatternKind>,
    data: Option<Rc<str>>,
}

impl Pattern {
    fn new() -> Self {
        Self {
            kind: None,
            data: None,
        }
    }

    fn parse(&mut self) -> Result<Self> {
        todo!()
    }
}
