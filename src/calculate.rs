use num_bigint::BigUint;
use num_traits::{One, Zero};
use regex_syntax::hir::{Class, Hir, HirKind};

use crate::error::{Error, Result};
use crate::model::Limits;

#[derive(Debug, Clone)]
pub(crate) enum Amount {
    Finite(BigUint),
    Infinite,
}

impl Amount {
    pub(crate) fn display(&self) -> String {
        match self {
            Self::Finite(n) => n.to_string(),
            Self::Infinite => "infinite".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Stats {
    pub count: Amount,
    pub total_bytes: Amount,
}

#[derive(Debug, Clone)]
struct Dist {
    items: Vec<(usize, BigUint)>,
    infinite: bool,
}

impl Dist {
    fn empty_string() -> Self {
        Self {
            items: vec![(0, BigUint::one())],
            infinite: false,
        }
    }

    fn none() -> Self {
        Self {
            items: Vec::new(),
            infinite: false,
        }
    }

    fn literal(len: usize) -> Self {
        Self {
            items: vec![(len, BigUint::one())],
            infinite: false,
        }
    }

    fn class(widths: &[usize]) -> Self {
        let mut items: Vec<(usize, BigUint)> = Vec::new();
        for &width in widths {
            add_count(&mut items, width, BigUint::one());
        }
        Self {
            items,
            infinite: false,
        }
    }

    fn filtered(&self, limits: &Limits) -> Self {
        Self {
            items: self
                .items
                .iter()
                .filter(|(len, _)| {
                    *len >= limits.min_len && limits.max_len.is_none_or(|max| *len <= max)
                })
                .cloned()
                .collect(),
            infinite: self.infinite && limits.max_len.is_none(),
        }
    }

    fn alternate(parts: &[Self]) -> Self {
        let mut out = Vec::new();
        let mut infinite = false;
        for part in parts {
            infinite |= part.infinite;
            for (len, count) in &part.items {
                add_count(&mut out, *len, count.clone());
            }
        }
        Self {
            items: out,
            infinite,
        }
    }

    fn concat(parts: &[Self], max_len: Option<usize>) -> Self {
        let mut acc = Self::empty_string();
        let mut infinite = false;
        for part in parts {
            infinite |= part.infinite;
            let mut next = Vec::new();
            for (left_len, left_count) in &acc.items {
                for (right_len, right_count) in &part.items {
                    let len = left_len + right_len;
                    if max_len.is_none_or(|max| len <= max) {
                        add_count(&mut next, len, left_count * right_count);
                    }
                }
            }
            acc.items = next;
        }
        acc.infinite |= infinite;
        acc
    }

    fn repeat(sub: &Self, min: u32, max: Option<u32>, max_len: Option<usize>) -> Self {
        if sub.infinite && max.is_none() {
            return Self {
                items: Vec::new(),
                infinite: true,
            };
        }
        if sub.items.is_empty() {
            return if min == 0 {
                Self::empty_string()
            } else {
                Self::none()
            };
        }

        let min = min as usize;
        let max_reps = max.map(|n| n as usize).or_else(|| {
            let min_sub_len = sub
                .items
                .iter()
                .map(|(len, _)| *len)
                .filter(|len| *len > 0)
                .min()?;
            max_len.map(|limit| limit / min_sub_len)
        });
        let Some(max_reps) = max_reps else {
            return Self {
                items: Vec::new(),
                infinite: true,
            };
        };

        let mut out = Vec::new();
        let mut current = Self::empty_string();
        for reps in 0..=max_reps {
            if reps >= min {
                for (len, count) in &current.items {
                    add_count(&mut out, *len, count.clone());
                }
            }
            if reps == max_reps {
                break;
            }
            current = Self::concat(&[current, sub.clone()], max_len);
        }
        Self {
            items: out,
            infinite: false,
        }
    }

    fn stats(&self) -> Stats {
        if self.infinite {
            return Stats {
                count: Amount::Infinite,
                total_bytes: Amount::Infinite,
            };
        }
        let mut count = BigUint::zero();
        let mut total = BigUint::zero();
        for (len, n) in &self.items {
            count += n;
            total += n * BigUint::from(*len);
        }
        Stats {
            count: Amount::Finite(count),
            total_bytes: Amount::Finite(total),
        }
    }
}

fn add_count(items: &mut Vec<(usize, BigUint)>, len: usize, count: BigUint) {
    if let Some((_, existing)) = items
        .iter_mut()
        .find(|(existing_len, _)| *existing_len == len)
    {
        *existing += count;
    } else {
        items.push((len, count));
        items.sort_by_key(|(len, _)| *len);
    }
}

pub(crate) fn analyze(hir: &Hir, limits: &Limits) -> Result<Stats> {
    Ok(distribution(hir, limits.max_len)?.filtered(limits).stats())
}

pub(crate) fn min_positive_len(hir: &Hir, max_len: usize) -> Result<usize> {
    distribution(hir, Some(max_len))?
        .items
        .iter()
        .map(|(len, _)| *len)
        .filter(|len| *len > 0)
        .min()
        .ok_or(Error::Unsupported("unbounded empty repetition"))
}

fn distribution(hir: &Hir, max_len: Option<usize>) -> Result<Dist> {
    Ok(match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => Dist::empty_string(),
        HirKind::Literal(lit) => Dist::literal(lit.0.len()),
        HirKind::Class(class) => Dist::class(&class_widths(class)?),
        HirKind::Capture(cap) => distribution(&cap.sub, max_len)?,
        HirKind::Concat(parts) => {
            let parts = parts
                .iter()
                .map(|part| distribution(part, max_len))
                .collect::<Result<Vec<_>>>()?;
            Dist::concat(&parts, max_len)
        }
        HirKind::Alternation(parts) => {
            let parts = parts
                .iter()
                .map(|part| distribution(part, max_len))
                .collect::<Result<Vec<_>>>()?;
            Dist::alternate(&parts)
        }
        HirKind::Repetition(rep) => {
            let sub = distribution(&rep.sub, max_len)?;
            Dist::repeat(&sub, rep.min, rep.max, max_len)
        }
    })
}

fn class_widths(class: &Class) -> Result<Vec<usize>> {
    let mut widths = Vec::new();
    match class {
        Class::Unicode(cls) => {
            for range in cls.ranges() {
                let mut c = range.start() as u32;
                let end = range.end() as u32;
                while c <= end {
                    if let Some(ch) = char::from_u32(c) {
                        widths.push(ch.len_utf8());
                    }
                    c += 1;
                }
            }
        }
        Class::Bytes(cls) => {
            for range in cls.ranges() {
                for b in range.start()..=range.end() {
                    if b.is_ascii() {
                        widths.push(1);
                    } else {
                        return Err(Error::Unsupported("non-UTF-8 byte classes"));
                    }
                }
            }
        }
    }
    Ok(widths)
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;
    use regex_syntax::Parser;

    use super::{Amount, analyze};
    use crate::model::Limits;

    fn hir(pattern: &str) -> regex_syntax::hir::Hir {
        Parser::new().parse(pattern).unwrap()
    }

    fn limits(min_len: usize, max_len: Option<usize>) -> Limits {
        Limits { min_len, max_len }
    }

    fn finite(amount: Amount) -> BigUint {
        match amount {
            Amount::Finite(n) => n,
            Amount::Infinite => panic!("expected finite amount"),
        }
    }

    #[test]
    fn counts_literals_alternation_and_classes() {
        let stats = analyze(&hir("a|bc|[de]"), &limits(0, None)).unwrap();
        assert_eq!(finite(stats.count), BigUint::from(4u8));
        assert_eq!(finite(stats.total_bytes), BigUint::from(5u8));
    }

    #[test]
    fn counts_bounded_repetition() {
        let stats = analyze(&hir("a{2,4}"), &limits(0, None)).unwrap();
        assert_eq!(finite(stats.count), BigUint::from(3u8));
        assert_eq!(finite(stats.total_bytes), BigUint::from(9u8));
    }

    #[test]
    fn counts_unicode_bytes() {
        let stats = analyze(&hir("é|a"), &limits(0, None)).unwrap();
        assert_eq!(finite(stats.count), BigUint::from(2u8));
        assert_eq!(finite(stats.total_bytes), BigUint::from(3u8));
    }

    #[test]
    fn reports_unbounded_repetition_as_infinite() {
        let stats = analyze(&hir("a*"), &limits(0, None)).unwrap();
        assert!(matches!(stats.count, Amount::Infinite));
        assert!(matches!(stats.total_bytes, Amount::Infinite));
    }

    #[test]
    fn length_filters_bound_unbounded_repetition() {
        let stats = analyze(&hir("a*"), &limits(1, Some(3))).unwrap();
        assert_eq!(finite(stats.count), BigUint::from(3u8));
        assert_eq!(finite(stats.total_bytes), BigUint::from(6u8));
    }
}
