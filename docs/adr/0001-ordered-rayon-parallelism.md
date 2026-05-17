# 0001 Ordered Rayon Parallelism

## Status

Superseded by [0002 Unordered Generation by Default](0002-unordered-generation-by-default.md)

## Context

Rexgen analyzes and emits the finite UTF-8 string corpus described by a regular expression. The CLI help and tests promise deterministic regex traversal order for generated Match Strings.

Large analysis and generation runs can spend most of their time in independent HIR branches or repeated-prefix expansion. Parallelizing those units can improve throughput, but unordered output would break existing scripting behavior.

## Decision

Use Rayon for internal parallel execution while preserving ordered generation output.

Analysis may evaluate independent HIR branches in parallel and then combine exact BigUint distributions deterministically. Generation may compute coarse ordered chunks in parallel, but chunks are drained through the existing output path in regex traversal order.

The CLI exposes `--threads N` for predictable runs. Without it, Rayon chooses the worker count.

## Consequences

Ordered output remains compatible with existing users and tests.

The implementation is intentionally conservative: if an ordered generation chunk grows beyond the internal buffer cap, Rexgen falls back to the existing sequential generator rather than materializing an unbounded amount of output.

Patterns with natural independent branches or fixed repeated prefixes benefit most. Runs dominated by a single serial branch or slow output I/O may see limited speedup.
