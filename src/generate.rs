use std::ops::Deref;
use std::ops::RangeInclusive;

use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use regex_syntax::hir::Class;
use regex_syntax::hir::Hir;
use regex_syntax::hir::HirKind;

use crate::error::Error;
use crate::error::Result;

#[derive(Debug, Default)]
pub(crate) struct Generate {}

impl Generate {
    pub(crate) fn new() -> Self {
        Self {}
    }

    pub(crate) fn hir(&self) -> Self {
        todo!()
    }

    pub(crate) fn generate(&self, hir: &Hir) -> Result<Vec<String>> {
        let mut results = Vec::new();

        match hir.kind() {
            HirKind::Empty => todo!(),
            HirKind::Literal(lit) => {
                results.push(String::from_utf8(lit.0.to_vec()).map_err(
                    |_| Error::Message("lasdkfjasldfkj".to_string()),
                )?)
            }
            HirKind::Class(class) => results.extend(self.gen_from_class(class)),
            _ => todo!(),
        }

        Ok(results)
    }

    fn gen_from_class(&self, class: &Class) -> Vec<String> {
        let mut chars = CharRanges::new();

        match class {
            Class::Unicode(uni) => {
                chars.extend(uni.iter().fold(
                    CharRanges::new(),
                    move |mut acc, r| {
                        acc.push(r.start()..=r.end());
                        acc
                    },
                ));
            }
            Class::Bytes(bytes) => {
                chars.extend(bytes.iter().fold(
                    CharRanges::new(),
                    move |mut acc, r| {
                        acc.push((r.start() as char)..=(r.end() as char));
                        acc
                    },
                ));
            }
        }

        Vec::new()
    }
}

pub(crate) type CharRanges = Vec<RangeInclusive<char>>;

pub(crate) trait GenerateFrom<T> {
    fn generate(self) -> Vec<String>;
}
