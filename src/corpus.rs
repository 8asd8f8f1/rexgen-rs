use num_bigint::BigUint;
use regex::Regex;
use regex_syntax::Parser;
use regex_syntax::hir::Hir;

use crate::calculate::Stats;
use crate::calculate::{self};
use crate::error::Error;
use crate::error::Result;
use crate::generate;
use crate::generate::GenerationOrder;

pub(crate) struct Corpus {
    hir: Hir,
    matcher: Regex,
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
    pub start_string: Option<String>,
    pub stop_string: Option<String>,
    pub reverse_strings: bool,
    pub order: GenerationOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationControl {
    Continue,
    #[allow(dead_code)]
    Stop,
}

impl Corpus {
    pub(crate) fn new(
        pattern: &str,
        constraints: LengthConstraints,
    ) -> Result<Self> {
        constraints.validate()?;
        let matcher = Regex::new(&format!("^(?:{pattern})$"))?;
        Ok(Self {
            hir: Parser::new().parse(pattern)?,
            matcher,
            constraints,
        })
    }

    pub(crate) fn constraints(&self) -> &LengthConstraints {
        &self.constraints
    }

    pub(crate) fn analyze(&self) -> Result<Stats> {
        calculate::analyze(&self.hir, &self.constraints)
    }

    pub(crate) fn generate<F>(
        &self,
        request: GenerationRequest,
        mut emit: F,
    ) -> Result<()>
    where
        F: FnMut(&str) -> Result<GenerationControl>,
    {
        self.validate_generation_request(&request)?;
        let constraints = self.generation_constraints(request.limit);
        let mut emitted = 0u64;
        let mut total = BigUint::from(0u8);
        let mut started = request.start_string.is_none();
        let mut stopped = false;

        generate::generate_parallel(
            &self.hir,
            &constraints,
            request.order,
            |s| {
                if stopped {
                    return Ok(false);
                }
                if !started {
                    if request.start_string.as_deref() == Some(s) {
                        started = true;
                    } else {
                        return Ok(true);
                    }
                }
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

                let output;
                let emitted_s = if request.reverse_strings {
                    output = s.chars().rev().collect::<String>();
                    output.as_str()
                } else {
                    s
                };
                let control = emit(emitted_s)?;
                emitted += 1;
                total = next_total;
                if request.stop_string.as_deref() == Some(s) {
                    stopped = true;
                }
                Ok(control == GenerationControl::Continue)
            },
        )
    }

    pub(crate) fn validate_generation_request(
        &self,
        request: &GenerationRequest,
    ) -> Result<()> {
        self.validate_bound("start string", request.start_string.as_deref())?;
        self.validate_bound("stop string", request.stop_string.as_deref())
    }

    fn validate_bound(&self, label: &str, value: Option<&str>) -> Result<()> {
        let Some(value) = value else {
            return Ok(());
        };
        if !self.matcher.is_match(value) {
            return Err(Error::Message(format!(
                "{label} must match the pattern"
            )));
        }
        let len = value.len();
        if len < self.constraints.min
            || self.constraints.max.is_some_and(|max| len > max)
        {
            return Err(Error::Message(format!(
                "{label} must satisfy length constraints"
            )));
        }
        Ok(())
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

    use super::Corpus;
    use super::GenerationControl;
    use super::GenerationRequest;
    use super::LengthConstraints;
    use crate::calculate::Amount;
    use crate::error::Result;
    use crate::generate::GenerationOrder;

    fn constraints(min: usize, max: Option<usize>) -> LengthConstraints {
        LengthConstraints { min, max }
    }

    fn request(
        limit: Option<u64>,
        max_total_bytes: Option<u64>,
    ) -> GenerationRequest {
        GenerationRequest {
            limit,
            max_total_bytes: max_total_bytes.map(BigUint::from),
            start_string: None,
            stop_string: None,
            reverse_strings: false,
            order: GenerationOrder::Default,
        }
    }

    fn generate_strings(
        pattern: &str,
        constraints: LengthConstraints,
        request: GenerationRequest,
    ) -> Result<Vec<String>> {
        let corpus = Corpus::new(pattern, constraints)?;
        let mut out = Vec::new();
        corpus.generate(request, |s| {
            out.push(s.to_string());
            Ok(GenerationControl::Continue)
        })?;
        Ok(out)
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

    #[test]
    fn generation_can_start_at_match_string() {
        let mut req = request(None, None);
        req.start_string = Some("ba".to_string());

        let out =
            generate_strings("[ab]{2}", constraints(0, None), req).unwrap();
        assert_eq!(out, vec!["ba", "bb"]);
    }

    #[test]
    fn generation_can_stop_at_match_string() {
        let mut req = request(None, None);
        req.stop_string = Some("ba".to_string());

        let out =
            generate_strings("[ab]{2}", constraints(0, None), req).unwrap();
        assert_eq!(out, vec!["aa", "ab", "ba"]);
    }

    #[test]
    fn generation_can_start_and_stop_at_match_strings() {
        let mut req = request(None, None);
        req.start_string = Some("ab".to_string());
        req.stop_string = Some("ba".to_string());

        let out =
            generate_strings("[ab]{2}", constraints(0, None), req).unwrap();
        assert_eq!(out, vec!["ab", "ba"]);
    }

    #[test]
    fn generation_rejects_bounds_outside_pattern() {
        let mut req = request(None, None);
        req.start_string = Some("ca".to_string());

        let err =
            generate_strings("[ab]{2}", constraints(0, None), req).unwrap_err();
        assert!(err.to_string().contains("must match the pattern"));
    }

    #[test]
    fn generation_rejects_bounds_outside_length_constraints() {
        let mut req = request(None, None);
        req.stop_string = Some("a".to_string());

        let err = generate_strings("[ab]{1,2}", constraints(2, None), req)
            .unwrap_err();
        assert!(err.to_string().contains("must satisfy length constraints"));
    }

    #[test]
    fn generation_can_reverse_emitted_strings() {
        let mut req = request(None, None);
        req.reverse_strings = true;

        let out = generate_strings("ab|éx", constraints(0, None), req).unwrap();
        assert_eq!(out, vec!["ba", "xé"]);
    }

    #[test]
    fn generation_reverse_does_not_change_analysis() {
        let corpus = Corpus::new("ab|cd", constraints(0, None)).unwrap();
        let stats = corpus.analyze().unwrap();

        let mut req = request(None, None);
        req.reverse_strings = true;
        let out = generate_strings("ab|cd", constraints(0, None), req).unwrap();

        assert_eq!(finite(stats.count), BigUint::from(2u8));
        assert_eq!(finite(stats.total_bytes), BigUint::from(4u8));
        assert_eq!(out, vec!["ba", "dc"]);
    }

    #[test]
    fn generation_can_invert_fixed_repetition_order() {
        let mut req = request(None, None);
        req.order = GenerationOrder::Inverted;

        let out =
            generate_strings("[ab]{2}", constraints(0, None), req).unwrap();
        assert_eq!(out, vec!["aa", "ba", "ab", "bb"]);
    }
}
