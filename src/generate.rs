use regex_syntax::hir::{Class, Hir, HirKind};

use crate::calculate;
use crate::corpus::LengthConstraints;
use crate::error::{Error, Result};

pub(crate) fn generate<F>(hir: &Hir, constraints: &LengthConstraints, mut emit: F) -> Result<()>
where
    F: FnMut(&str) -> Result<bool>,
{
    let mut current = String::new();
    generate_inner(hir, constraints, &mut current, &mut emit).map(|_| ())
}

fn generate_inner<F>(
    hir: &Hir,
    constraints: &LengthConstraints,
    current: &mut String,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => emit_if_allowed(current, constraints, emit),
        HirKind::Literal(lit) => {
            let s = std::str::from_utf8(&lit.0)
                .map_err(|_| Error::Unsupported("non-UTF-8 literals"))?;
            current.push_str(s);
            let keep_going = emit_if_allowed(current, constraints, emit)?;
            truncate_str(current, s.len());
            Ok(keep_going)
        }
        HirKind::Class(class) => {
            for s in class_strings(class)? {
                current.push_str(&s);
                let keep_going = emit_if_allowed(current, constraints, emit)?;
                truncate_str(current, s.len());
                if !keep_going {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        HirKind::Capture(cap) => generate_inner(&cap.sub, constraints, current, emit),
        HirKind::Alternation(parts) => {
            for part in parts {
                if !generate_inner(part, constraints, current, emit)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        HirKind::Concat(parts) => generate_concat(parts, constraints, current, emit),
        HirKind::Repetition(rep) => {
            let max = match rep.max {
                Some(max) => max,
                None => repetition_cap(
                    &rep.sub,
                    constraints.max.ok_or_else(|| {
                        Error::Message("infinite generation requires --max-len".to_string())
                    })?,
                )?,
            };
            generate_repeat(&rep.sub, rep.min, max, constraints, current, emit)
        }
    }
}

fn repetition_cap(sub: &Hir, max_len: usize) -> Result<u32> {
    let min_len = calculate::min_positive_len(sub, max_len)?;
    Ok((max_len / min_len) as u32)
}

fn emit_if_allowed<F>(current: &str, constraints: &LengthConstraints, emit: &mut F) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    let len = current.len();
    if len < constraints.min || constraints.max.is_some_and(|max| len > max) {
        return Ok(true);
    }
    emit(current)
}

fn generate_concat<F>(
    parts: &[Hir],
    constraints: &LengthConstraints,
    current: &mut String,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    fn rec<F>(
        parts: &[Hir],
        constraints: &LengthConstraints,
        current: &mut String,
        emit: &mut F,
    ) -> Result<bool>
    where
        F: FnMut(&str) -> Result<bool>,
    {
        if parts.is_empty() {
            return emit_if_allowed(current, constraints, emit);
        }
        let start_len = current.len();
        let mut keep = true;
        generate_prefixes(
            &parts[0],
            constraints.max,
            current.len(),
            &mut |candidate| {
                let saved = candidate.to_string();
                current.push_str(&saved);
                keep = rec(&parts[1..], constraints, current, emit)?;
                truncate_str(current, saved.len());
                Ok(keep)
            },
        )?;
        current.truncate(start_len);
        Ok(keep)
    }
    rec(parts, constraints, current, emit)
}

fn generate_repeat<F>(
    sub: &Hir,
    min: u32,
    max: u32,
    constraints: &LengthConstraints,
    current: &mut String,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    fn rec<F>(
        sub: &Hir,
        reps: u32,
        min: u32,
        max: u32,
        constraints: &LengthConstraints,
        current: &mut String,
        emit: &mut F,
    ) -> Result<bool>
    where
        F: FnMut(&str) -> Result<bool>,
    {
        if reps >= min && !emit_if_allowed(current, constraints, emit)? {
            return Ok(false);
        }
        if reps == max {
            return Ok(true);
        }
        let mut keep = true;
        generate_prefixes(sub, constraints.max, current.len(), &mut |piece| {
            if piece.is_empty() {
                return Ok(true);
            }
            current.push_str(piece);
            keep = rec(sub, reps + 1, min, max, constraints, current, emit)?;
            truncate_str(current, piece.len());
            Ok(keep)
        })?;
        Ok(keep)
    }
    rec(sub, 0, min, max, constraints, current, emit)
}

fn generate_prefixes<F>(
    hir: &Hir,
    max_len: Option<usize>,
    current_len: usize,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    let mut out = Vec::new();
    collect_strings(
        hir,
        max_len.map(|max| max.saturating_sub(current_len)),
        &mut out,
    )?;
    for s in out {
        if !emit(&s)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn collect_strings(hir: &Hir, max_len: Option<usize>, out: &mut Vec<String>) -> Result<()> {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => out.push(String::new()),
        HirKind::Literal(lit) => {
            let s = std::str::from_utf8(&lit.0)
                .map_err(|_| Error::Unsupported("non-UTF-8 literals"))?;
            if max_len.is_none_or(|max| s.len() <= max) {
                out.push(s.to_string());
            }
        }
        HirKind::Class(class) => {
            for s in class_strings(class)? {
                if max_len.is_none_or(|max| s.len() <= max) {
                    out.push(s);
                }
            }
        }
        HirKind::Capture(cap) => collect_strings(&cap.sub, max_len, out)?,
        HirKind::Alternation(parts) => {
            for part in parts {
                collect_strings(part, max_len, out)?;
            }
        }
        HirKind::Concat(parts) => {
            let mut acc = vec![String::new()];
            for part in parts {
                let mut pieces = Vec::new();
                collect_strings(part, max_len, &mut pieces)?;
                let mut next = Vec::new();
                for left in &acc {
                    for right in &pieces {
                        let s = format!("{left}{right}");
                        if max_len.is_none_or(|max| s.len() <= max) {
                            next.push(s);
                        }
                    }
                }
                acc = next;
            }
            out.extend(acc);
        }
        HirKind::Repetition(rep) => {
            let max = rep.max.ok_or(Error::Unsupported(
                "unbounded repetition in nested generation",
            ))?;
            collect_repeat_strings(&rep.sub, rep.min, max, max_len, out)?;
        }
    }
    Ok(())
}

fn collect_repeat_strings(
    sub: &Hir,
    min: u32,
    max: u32,
    max_len: Option<usize>,
    out: &mut Vec<String>,
) -> Result<()> {
    fn rec(
        sub_strings: &[String],
        min: u32,
        max: u32,
        reps: u32,
        current: &mut String,
        max_len: Option<usize>,
        out: &mut Vec<String>,
    ) {
        if reps >= min {
            out.push(current.clone());
        }
        if reps == max {
            return;
        }
        for piece in sub_strings {
            if piece.is_empty() {
                continue;
            }
            if max_len.is_some_and(|max| current.len() + piece.len() > max) {
                continue;
            }
            current.push_str(piece);
            rec(sub_strings, min, max, reps + 1, current, max_len, out);
            truncate_str(current, piece.len());
        }
    }

    let mut sub_strings = Vec::new();
    collect_strings(sub, max_len, &mut sub_strings)?;
    let mut current = String::new();
    rec(&sub_strings, min, max, 0, &mut current, max_len, out);
    Ok(())
}

fn class_strings(class: &Class) -> Result<Vec<String>> {
    let mut out = Vec::new();
    match class {
        Class::Unicode(cls) => {
            for range in cls.ranges() {
                let mut c = range.start() as u32;
                let end = range.end() as u32;
                while c <= end {
                    if let Some(ch) = char::from_u32(c) {
                        out.push(ch.to_string());
                    }
                    c += 1;
                }
            }
        }
        Class::Bytes(cls) => {
            for range in cls.ranges() {
                for b in range.start()..=range.end() {
                    if b.is_ascii() {
                        out.push(char::from(b).to_string());
                    } else {
                        return Err(Error::Unsupported("non-UTF-8 byte classes"));
                    }
                }
            }
        }
    }
    Ok(out)
}

fn truncate_str(s: &mut String, bytes: usize) {
    let new_len = s.len() - bytes;
    s.truncate(new_len);
}

#[cfg(test)]
mod tests {
    use regex_syntax::Parser;

    use super::generate;
    use crate::corpus::LengthConstraints;

    fn hir(pattern: &str) -> regex_syntax::hir::Hir {
        Parser::new().parse(pattern).unwrap()
    }

    fn constraints(min: usize, max: Option<usize>) -> LengthConstraints {
        LengthConstraints { min, max }
    }

    #[test]
    fn generates_in_regex_order() {
        let mut out = Vec::new();
        generate(&hir("a|b{1,2}"), &constraints(0, None), |s| {
            out.push(s.to_string());
            Ok(true)
        })
        .unwrap();
        assert_eq!(out, vec!["a", "b", "bb"]);
    }
}
