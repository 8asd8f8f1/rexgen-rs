use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc;

use bumpalo::Bump;
use bumpalo::collections::Vec as BumpVec;
use rayon::prelude::*;
use regex_syntax::hir::Class;
use regex_syntax::hir::Hir;
use regex_syntax::hir::HirKind;

use crate::calculate;
use crate::corpus::LengthConstraints;
use crate::error::Error;
use crate::error::Result;

const PARALLEL_CHUNK_BUFFER_BYTES: usize = 8 * 1024 * 1024;
const UNORDERED_CHUNK_BYTES: usize = 256 * 1024;
const UNORDERED_QUEUE_CHUNKS: usize = 16;

enum GenerationChunk<'a> {
    Hir(&'a Hir),
    RepetitionTail {
        sub: &'a Hir,
        min: u32,
        max: u32,
        prefix: String,
    },
}

type ArenaStrings<'a> = BumpVec<'a, &'a str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationOrder {
    Unordered,
    Default,
    Inverted,
}

impl GenerationOrder {
    pub(crate) fn is_ordered(self) -> bool {
        matches!(self, Self::Default | Self::Inverted)
    }
}

pub(crate) fn generate<F>(
    hir: &Hir,
    constraints: &LengthConstraints,
    mut emit: F,
) -> Result<()>
where
    F: FnMut(&str) -> Result<bool>,
{
    let mut current = String::new();
    generate_inner(
        hir,
        constraints,
        GenerationOrder::Default,
        &mut current,
        &mut emit,
    )
    .map(|_| ())
}

pub(crate) fn generate_parallel<F>(
    hir: &Hir,
    constraints: &LengthConstraints,
    order: GenerationOrder,
    mut emit: F,
) -> Result<()>
where
    F: FnMut(&str) -> Result<bool>,
{
    if order == GenerationOrder::Inverted {
        let mut current = String::new();
        return generate_inner(
            hir,
            constraints,
            order,
            &mut current,
            &mut emit,
        )
        .map(|_| ());
    }
    if rayon::current_num_threads() <= 1 {
        return generate(hir, constraints, emit);
    }
    let Some(chunks) = generation_chunks(hir, constraints)? else {
        return generate(hir, constraints, emit);
    };
    if chunks.len() <= 1 {
        return generate(hir, constraints, emit);
    }

    let generated = chunks
        .par_iter()
        .map(|chunk| collect_chunk(chunk, constraints))
        .collect::<Result<Vec<_>>>()?;

    if generated.iter().any(|chunk| chunk.overflowed) {
        return generate(hir, constraints, emit);
    }

    for chunk in generated {
        for s in chunk.strings {
            if !emit(&s)? {
                return Ok(());
            }
        }
    }
    Ok(())
}

pub(crate) fn generate_unordered_file<W>(
    hir: &Hir,
    constraints: &LengthConstraints,
    reverse_strings: bool,
    writer: &mut W,
) -> Result<bool>
where
    W: Write + Send,
{
    if rayon::current_num_threads() <= 1 {
        return Ok(false);
    }
    let Some(chunks) = generation_chunks(hir, constraints)? else {
        return Ok(false);
    };
    if chunks.len() <= 1 {
        return Ok(false);
    }

    let (tx, rx) =
        mpsc::sync_channel::<Result<Vec<u8>>>(UNORDERED_QUEUE_CHUNKS);
    let cancelled = Arc::new(AtomicBool::new(false));

    rayon::scope(|scope| {
        for chunk in chunks {
            let tx = tx.clone();
            let cancelled = Arc::clone(&cancelled);
            scope.spawn(move |_| {
                if cancelled.load(Ordering::Relaxed) {
                    return;
                }
                let result = collect_chunk_lines(
                    &chunk,
                    constraints,
                    reverse_strings,
                    &cancelled,
                    &tx,
                );
                if let Err(err) = result {
                    cancelled.store(true, Ordering::Relaxed);
                    let _ = tx.send(Err(err));
                }
            });
        }
        drop(tx);

        for chunk in rx {
            match chunk {
                Ok(bytes) => writer.write_all(&bytes)?,
                Err(err) => return Err(err),
            }
        }
        Ok(())
    })?;
    Ok(true)
}

fn collect_chunk_lines(
    chunk: &GenerationChunk<'_>,
    constraints: &LengthConstraints,
    reverse_strings: bool,
    cancelled: &AtomicBool,
    tx: &mpsc::SyncSender<Result<Vec<u8>>>,
) -> Result<()> {
    let mut out = Vec::with_capacity(UNORDERED_CHUNK_BYTES);
    let mut emit = |s: &str| {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(false);
        }
        append_line(&mut out, s, reverse_strings);
        if out.len() >= UNORDERED_CHUNK_BYTES {
            let next = Vec::with_capacity(UNORDERED_CHUNK_BYTES);
            let full = std::mem::replace(&mut out, next);
            if tx.send(Ok(full)).is_err() {
                cancelled.store(true, Ordering::Relaxed);
                return Ok(false);
            }
        }
        Ok(true)
    };

    match chunk {
        GenerationChunk::Hir(hir) => {
            let mut current = String::new();
            generate_inner(
                hir,
                constraints,
                GenerationOrder::Default,
                &mut current,
                &mut emit,
            )?;
        }
        GenerationChunk::RepetitionTail {
            sub,
            min,
            max,
            prefix,
        } => {
            let mut current = prefix.clone();
            generate_repeat_rec(
                sub,
                1,
                *min,
                *max,
                constraints,
                &mut current,
                &mut emit,
            )?;
        }
    }

    if !out.is_empty() && !cancelled.load(Ordering::Relaxed) {
        if tx.send(Ok(out)).is_err() {
            cancelled.store(true, Ordering::Relaxed);
        }
    }
    Ok(())
}

fn append_line(out: &mut Vec<u8>, s: &str, reverse_strings: bool) {
    if reverse_strings {
        let start = out.len();
        for ch in s.chars().rev() {
            let mut bytes = [0; 4];
            out.extend_from_slice(ch.encode_utf8(&mut bytes).as_bytes());
        }
        if out.len() == start {
            // Keep empty Match Strings newline-delimited.
        }
    } else {
        out.extend_from_slice(s.as_bytes());
    }
    out.push(b'\n');
}

struct ChunkOutput {
    strings: Vec<String>,
    overflowed: bool,
}

fn collect_chunk(
    chunk: &GenerationChunk<'_>,
    constraints: &LengthConstraints,
) -> Result<ChunkOutput> {
    let mut strings = Vec::new();
    let mut bytes = 0usize;
    let mut overflowed = false;
    let mut emit = |s: &str| {
        bytes = bytes.saturating_add(s.len());
        if bytes > PARALLEL_CHUNK_BUFFER_BYTES {
            overflowed = true;
            return Ok(false);
        }
        strings.push(s.to_string());
        Ok(true)
    };

    match chunk {
        GenerationChunk::Hir(hir) => {
            let mut current = String::new();
            generate_inner(
                hir,
                constraints,
                GenerationOrder::Default,
                &mut current,
                &mut emit,
            )?;
        }
        GenerationChunk::RepetitionTail {
            sub,
            min,
            max,
            prefix,
        } => {
            let mut current = prefix.clone();
            generate_repeat_rec(
                sub,
                1,
                *min,
                *max,
                constraints,
                &mut current,
                &mut emit,
            )?;
        }
    }

    Ok(ChunkOutput {
        strings,
        overflowed,
    })
}

fn generation_chunks<'a>(
    hir: &'a Hir,
    constraints: &LengthConstraints,
) -> Result<Option<Vec<GenerationChunk<'a>>>> {
    match hir.kind() {
        HirKind::Alternation(parts) => {
            Ok(Some(parts.iter().map(GenerationChunk::Hir).collect()))
        }
        HirKind::Repetition(rep) if rep.min > 0 => {
            let max = match rep.max {
                Some(max) => max,
                None => {
                    let Some(max_len) = constraints.max else {
                        return Ok(None);
                    };
                    repetition_cap(&rep.sub, max_len)?
                }
            };
            if rep.min != max {
                return Ok(None);
            }
            let arena = Bump::new();
            let mut pieces = BumpVec::new_in(&arena);
            collect_strings(&rep.sub, constraints.max, &arena, &mut pieces)?;
            let chunks = pieces
                .iter()
                .copied()
                .filter(|piece| !piece.is_empty())
                .map(|prefix| GenerationChunk::RepetitionTail {
                    sub: &rep.sub,
                    min: rep.min,
                    max,
                    prefix: prefix.to_string(),
                })
                .collect::<Vec<_>>();
            Ok(Some(chunks))
        }
        _ => Ok(None),
    }
}

fn generate_inner<F>(
    hir: &Hir,
    constraints: &LengthConstraints,
    order: GenerationOrder,
    current: &mut String,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => {
            emit_if_allowed(current, constraints, emit)
        }
        HirKind::Literal(lit) => {
            let s = std::str::from_utf8(&lit.0)
                .map_err(|_| Error::Unsupported("non-UTF-8 literals"))?;
            current.push_str(s);
            let keep_going = emit_if_allowed(current, constraints, emit)?;
            truncate_str(current, s.len());
            Ok(keep_going)
        }
        HirKind::Class(class) => {
            let mut keep_going = true;
            class_strings(class, &mut |s| {
                current.push_str(s);
                keep_going = emit_if_allowed(current, constraints, emit)?;
                truncate_str(current, s.len());
                if !keep_going {
                    return Ok(false);
                }
                Ok(true)
            })?;
            Ok(keep_going)
        }
        HirKind::Capture(cap) => {
            generate_inner(&cap.sub, constraints, order, current, emit)
        }
        HirKind::Alternation(parts) => {
            for part in parts {
                if !generate_inner(part, constraints, order, current, emit)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        HirKind::Concat(parts) => {
            generate_concat(parts, constraints, order, current, emit)
        }
        HirKind::Repetition(rep) => {
            let max = match rep.max {
                Some(max) => max,
                None => repetition_cap(
                    &rep.sub,
                    constraints.max.ok_or_else(|| {
                        Error::Message(
                            "infinite generation requires --max-len"
                                .to_string(),
                        )
                    })?,
                )?,
            };
            generate_repeat(
                &rep.sub,
                rep.min,
                max,
                constraints,
                order,
                current,
                emit,
            )
        }
    }
}

fn repetition_cap(sub: &Hir, max_len: usize) -> Result<u32> {
    let min_len = calculate::min_positive_len(sub, max_len)?;
    Ok((max_len / min_len) as u32)
}

fn emit_if_allowed<F>(
    current: &str,
    constraints: &LengthConstraints,
    emit: &mut F,
) -> Result<bool>
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
    order: GenerationOrder,
    current: &mut String,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    fn rec<F>(
        parts: &[Hir],
        constraints: &LengthConstraints,
        order: GenerationOrder,
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
        match order {
            GenerationOrder::Unordered | GenerationOrder::Default => {
                generate_prefixes(
                    &parts[0],
                    constraints.max,
                    current.len(),
                    &mut |candidate| {
                        current.push_str(candidate);
                        keep = rec(
                            &parts[1..],
                            constraints,
                            order,
                            current,
                            emit,
                        )?;
                        truncate_str(current, candidate.len());
                        Ok(keep)
                    },
                )?;
            }
            GenerationOrder::Inverted => {
                if parts.len() == 1 {
                    keep = generate_inner(
                        &parts[0],
                        constraints,
                        order,
                        current,
                        emit,
                    )?;
                } else {
                    let last = parts.len() - 1;
                    generate_prefixes(
                        &parts[last],
                        constraints.max,
                        current.len(),
                        &mut |candidate| {
                            let mut left_parts = parts[..last].to_vec();
                            left_parts.push(Hir::literal(candidate.as_bytes()));
                            keep = rec(
                                &left_parts,
                                constraints,
                                order,
                                current,
                                emit,
                            )?;
                            Ok(keep)
                        },
                    )?;
                }
            }
        }
        current.truncate(start_len);
        Ok(keep)
    }
    rec(parts, constraints, order, current, emit)
}

fn generate_repeat<F>(
    sub: &Hir,
    min: u32,
    max: u32,
    constraints: &LengthConstraints,
    order: GenerationOrder,
    current: &mut String,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    if order == GenerationOrder::Inverted {
        if min != max {
            return Err(Error::Unsupported(
                "inverted order for variable repetition",
            ));
        }
        return generate_fixed_repeat_inverted(
            sub,
            max,
            constraints,
            current,
            emit,
        );
    }
    generate_repeat_rec(sub, 0, min, max, constraints, current, emit)
}

fn generate_fixed_repeat_inverted<F>(
    sub: &Hir,
    reps: u32,
    constraints: &LengthConstraints,
    current: &mut String,
    emit: &mut F,
) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    fn rec<'a, F>(
        pieces: &[&'a str],
        slots: &mut [&'a str],
        slot: isize,
        constraints: &LengthConstraints,
        current: &mut String,
        emit: &mut F,
    ) -> Result<bool>
    where
        F: FnMut(&str) -> Result<bool>,
    {
        if slot < 0 {
            let start_len = current.len();
            for piece in slots.iter() {
                current.push_str(piece);
            }
            let keep = emit_if_allowed(current, constraints, emit)?;
            current.truncate(start_len);
            return Ok(keep);
        }

        for piece in pieces {
            slots[slot as usize] = piece;
            if !rec(pieces, slots, slot - 1, constraints, current, emit)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    let arena = Bump::new();
    let mut pieces = BumpVec::new_in(&arena);
    collect_strings(sub, constraints.max, &arena, &mut pieces)?;
    if pieces.iter().any(|piece| piece.is_empty()) {
        return Err(Error::Unsupported("inverted order for empty repetition"));
    }
    let pieces = pieces.into_iter().collect::<Vec<_>>();
    let mut slots = vec![""; reps as usize];
    rec(
        &pieces,
        &mut slots,
        reps as isize - 1,
        constraints,
        current,
        emit,
    )
}

fn generate_repeat_rec<F>(
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
        keep = generate_repeat_rec(
            sub,
            reps + 1,
            min,
            max,
            constraints,
            current,
            emit,
        )?;
        truncate_str(current, piece.len());
        Ok(keep)
    })?;
    Ok(keep)
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
    let arena = Bump::new();
    let mut out = BumpVec::new_in(&arena);
    collect_strings(
        hir,
        max_len.map(|max| max.saturating_sub(current_len)),
        &arena,
        &mut out,
    )?;
    for s in out.into_iter() {
        if !emit(s)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn collect_strings<'a>(
    hir: &'a Hir,
    max_len: Option<usize>,
    arena: &'a Bump,
    out: &mut ArenaStrings<'a>,
) -> Result<()> {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => out.push(""),
        HirKind::Literal(lit) => {
            let s = std::str::from_utf8(&lit.0)
                .map_err(|_| Error::Unsupported("non-UTF-8 literals"))?;
            if max_len.is_none_or(|max| s.len() <= max) {
                out.push(s);
            }
        }
        HirKind::Class(class) => {
            class_strings(class, &mut |s| {
                if max_len.is_none_or(|max| s.len() <= max) {
                    out.push(arena.alloc_str(s));
                }
                Ok(true)
            })?;
        }
        HirKind::Capture(cap) => {
            collect_strings(&cap.sub, max_len, arena, out)?
        }
        HirKind::Alternation(parts) => {
            for part in parts {
                collect_strings(part, max_len, arena, out)?;
            }
        }
        HirKind::Concat(parts) => {
            let mut acc = BumpVec::new_in(arena);
            acc.push("");
            for part in parts {
                let mut pieces = BumpVec::new_in(arena);
                collect_strings(part, max_len, arena, &mut pieces)?;
                let mut next = BumpVec::new_in(arena);
                for left in &acc {
                    for right in &pieces {
                        let len = left.len() + right.len();
                        if max_len.is_none_or(|max| len <= max) {
                            let mut s = String::with_capacity(len);
                            s.push_str(left);
                            s.push_str(right);
                            next.push(&*arena.alloc_str(&s));
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
            collect_repeat_strings(
                &rep.sub, rep.min, max, max_len, arena, out,
            )?;
        }
    }
    Ok(())
}

fn collect_repeat_strings<'a>(
    sub: &'a Hir,
    min: u32,
    max: u32,
    max_len: Option<usize>,
    arena: &'a Bump,
    out: &mut ArenaStrings<'a>,
) -> Result<()> {
    fn rec<'a>(
        sub_strings: &[&str],
        min: u32,
        max: u32,
        reps: u32,
        current: &mut String,
        max_len: Option<usize>,
        arena: &'a Bump,
        out: &mut ArenaStrings<'a>,
    ) {
        if reps >= min {
            out.push(arena.alloc_str(current));
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
            rec(
                sub_strings,
                min,
                max,
                reps + 1,
                current,
                max_len,
                arena,
                out,
            );
            truncate_str(current, piece.len());
        }
    }

    let mut sub_strings = BumpVec::new_in(arena);
    collect_strings(sub, max_len, arena, &mut sub_strings)?;
    let sub_strings = sub_strings.into_iter().collect::<Vec<_>>();
    let mut current = String::new();
    rec(&sub_strings, min, max, 0, &mut current, max_len, arena, out);
    Ok(())
}

fn class_strings<F>(class: &Class, emit: &mut F) -> Result<bool>
where
    F: FnMut(&str) -> Result<bool>,
{
    match class {
        Class::Unicode(cls) => {
            for range in cls.ranges() {
                let mut c = range.start() as u32;
                let end = range.end() as u32;
                while c <= end {
                    if let Some(ch) = char::from_u32(c) {
                        let mut bytes = [0; 4];
                        if !emit(ch.encode_utf8(&mut bytes))? {
                            return Ok(false);
                        }
                    }
                    c += 1;
                }
            }
        }
        Class::Bytes(cls) => {
            for range in cls.ranges() {
                for b in range.start()..=range.end() {
                    if b.is_ascii() {
                        let bytes = [b];
                        let s = std::str::from_utf8(&bytes).map_err(|_| {
                            Error::Unsupported("non-UTF-8 byte classes")
                        })?;
                        if !emit(s)? {
                            return Ok(false);
                        }
                    } else {
                        return Err(Error::Unsupported(
                            "non-UTF-8 byte classes",
                        ));
                    }
                }
            }
        }
    }
    Ok(true)
}

fn truncate_str(s: &mut String, bytes: usize) {
    let new_len = s.len() - bytes;
    s.truncate(new_len);
}

#[cfg(test)]
mod tests {
    use regex_syntax::Parser;

    use super::GenerationOrder;
    use super::generate;
    use super::generate_parallel;
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

    #[test]
    fn parallel_generation_preserves_alternation_order() {
        let mut out = Vec::new();
        generate_parallel(
            &hir("a|b{1,2}|c"),
            &constraints(0, None),
            GenerationOrder::Default,
            |s| {
                out.push(s.to_string());
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(out, vec!["a", "b", "bb", "c"]);
    }

    #[test]
    fn parallel_generation_preserves_fixed_repetition_order() {
        let mut out = Vec::new();
        generate_parallel(
            &hir("[ab]{2}"),
            &constraints(0, None),
            GenerationOrder::Default,
            |s| {
                out.push(s.to_string());
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(out, vec!["aa", "ab", "ba", "bb"]);
    }

    #[test]
    fn parallel_generation_honors_callback_stop_in_output_order() {
        let mut out = Vec::new();
        generate_parallel(
            &hir("[ab]{2}"),
            &constraints(0, None),
            GenerationOrder::Default,
            |s| {
                out.push(s.to_string());
                Ok(out.len() < 2)
            },
        )
        .unwrap();
        assert_eq!(out, vec!["aa", "ab"]);
    }

    #[test]
    fn generates_unicode_classes_after_arena_collection() {
        let mut out = Vec::new();
        generate(&hir("[éa]{2}"), &constraints(0, None), |s| {
            out.push(s.to_string());
            Ok(true)
        })
        .unwrap();
        assert_eq!(
            out,
            vec!["a", "é"]
                .into_iter()
                .flat_map(|left| {
                    ["a", "é"]
                        .into_iter()
                        .map(move |right| format!("{left}{right}"))
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn generates_concat_and_repetition_after_arena_collection() {
        let mut out = Vec::new();
        generate(&hir("(ab|c)[12]{1,2}"), &constraints(0, None), |s| {
            out.push(s.to_string());
            Ok(true)
        })
        .unwrap();
        assert_eq!(
            out,
            vec![
                "ab1", "ab11", "ab12", "ab2", "ab21", "ab22", "c1", "c11",
                "c12", "c2", "c21", "c22",
            ]
        );
    }
}
