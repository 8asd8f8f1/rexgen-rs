use num_bigint::BigUint;
use regex_syntax::{Parser, hir::Hir};

use crate::calculate::{self, Stats};
use crate::error::{Error, Result};
use crate::generate;

pub(crate) struct Corpus {
    hir: Hir,
    constraints: LengthConstraints,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LengthConstraints {
    pub min: usize,
    pub max: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct GenerationRequest {
    pub limit: Option<u64>,
    pub max_total_bytes: Option<BigUint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationControl {
    Continue,
    #[allow(dead_code)]
    Stop,
}

impl Corpus {
    pub(crate) fn new(pattern: &str, constraints: LengthConstraints) -> Result<Self> {
        constraints.validate()?;
        Ok(Self {
            hir: Parser::new().parse(pattern)?,
            constraints,
        })
    }

    pub(crate) fn constraints(&self) -> &LengthConstraints {
        &self.constraints
    }

    pub(crate) fn analyze(&self) -> Result<Stats> {
        calculate::analyze(&self.hir, &self.constraints)
    }

    pub(crate) fn generate<F>(&self, request: GenerationRequest, mut emit: F) -> Result<()>
    where
        F: FnMut(&str) -> Result<GenerationControl>,
    {
        let constraints = self.generation_constraints(request.limit);
        let mut emitted = 0u64;
        let mut total = BigUint::from(0u8);

        generate::generate(&self.hir, &constraints, |s| {
            if request.limit.is_some_and(|limit| emitted >= limit) {
                return Ok(false);
            }
            let next_total = &total + BigUint::from(s.len());
            if request
                .max_total_bytes
                .as_ref()
                .is_some_and(|max| next_total > *max)
            {
                return Ok(false);
            }

            let control = emit(s)?;
            emitted += 1;
            total = next_total;
            Ok(control == GenerationControl::Continue)
        })
    }

    fn generation_constraints(&self, limit: Option<u64>) -> LengthConstraints {
        if self.constraints.max.is_some() || limit.is_none() {
            return self.constraints.clone();
        }
        let min = self.hir.properties().minimum_len().unwrap_or(0);
        LengthConstraints {
            min: self.constraints.min,
            max: Some(min.saturating_add(limit.unwrap() as usize)),
        }
    }
}

impl LengthConstraints {
    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(max) = self.max {
            if self.min > max {
                return Err(Error::Message(
                    "--min-len cannot exceed --max-len".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::{Corpus, GenerationControl, GenerationRequest, LengthConstraints};
    use crate::calculate::Amount;

    fn constraints(min: usize, max: Option<usize>) -> LengthConstraints {
        LengthConstraints { min, max }
    }

    fn request(limit: Option<u64>, max_total_bytes: Option<u64>) -> GenerationRequest {
        GenerationRequest {
            limit,
            max_total_bytes: max_total_bytes.map(BigUint::from),
        }
    }

    fn finite(amount: Amount) -> BigUint {
        match amount {
            Amount::Finite(n) => n,
            Amount::Infinite => panic!("expected finite amount"),
        }
    }

    #[test]
    fn analyzes_literals_alternation_and_classes() {
        let corpus = Corpus::new("a|bc|[de]", constraints(0, None)).unwrap();
        let stats = corpus.analyze().unwrap();
        assert_eq!(finite(stats.count), BigUint::from(4u8));
        assert_eq!(finite(stats.total_bytes), BigUint::from(5u8));
    }

    #[test]
    fn analyzes_bounded_unbounded_repetition() {
        let corpus = Corpus::new("a*", constraints(1, Some(3))).unwrap();
        let stats = corpus.analyze().unwrap();
        assert_eq!(finite(stats.count), BigUint::from(3u8));
        assert_eq!(finite(stats.total_bytes), BigUint::from(6u8));
    }

    #[test]
    fn reports_unbounded_corpus_as_infinite() {
        let corpus = Corpus::new("a*", constraints(0, None)).unwrap();
        let stats = corpus.analyze().unwrap();
        assert!(matches!(stats.count, Amount::Infinite));
        assert!(matches!(stats.total_bytes, Amount::Infinite));
    }

    #[test]
    fn generates_match_strings_in_regex_order() {
        let corpus = Corpus::new("a|b{1,2}", constraints(0, None)).unwrap();
        let mut out = Vec::new();
        corpus
            .generate(request(None, None), |s| {
                out.push(s.to_string());
                Ok(GenerationControl::Continue)
            })
            .unwrap();
        assert_eq!(out, vec!["a", "b", "bb"]);
    }

    #[test]
    fn generation_caps_do_not_change_analysis() {
        let corpus = Corpus::new("[ab]{1,3}", constraints(0, None)).unwrap();
        let stats = corpus.analyze().unwrap();

        let mut out = Vec::new();
        corpus
            .generate(request(Some(2), Some(4)), |s| {
                out.push(s.to_string());
                Ok(GenerationControl::Continue)
            })
            .unwrap();

        assert_eq!(finite(stats.count), BigUint::from(14u8));
        assert_eq!(out, vec!["a", "aa"]);
    }

    #[test]
    fn generation_callback_can_stop_emission() {
        let corpus = Corpus::new("[ab]{1,2}", constraints(0, None)).unwrap();
        let mut out = Vec::new();

        corpus
            .generate(request(None, None), |s| {
                out.push(s.to_string());
                Ok(GenerationControl::Stop)
            })
            .unwrap();

        assert_eq!(out, vec!["a"]);
    }
}
