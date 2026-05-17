# 0002 Unordered Generation by Default

## Status

Accepted

## Context

Rexgen emits Match Strings for a Corpus. ADR 0001 chose ordered Rayon parallelism so generated output stayed in deterministic regex traversal order.

Generation throughput is now the primary performance goal. Preserving global order forces worker output through ordered buffers and can require materializing many Match Strings before writing them. That increases allocation, memory pressure, and latency before file output receives data.

Some generation features remain order-sensitive: ordered generation itself, inverted Generation Order, Start Match String, and Stop Match String.

## Decision

Make unordered generation the default for file output when no order-sensitive generation option is active.

Expose `--ordered` for deterministic regex traversal order. Treat `--invert-order`, `--start-string`, and `--stop-string` as ordered generation requests.

Use bounded worker byte chunks and a single writer path for unordered file output. Workers may produce chunks independently, but each chunk contains complete newline-delimited Match Strings. The bounded queue applies backpressure when writing is slower than generation.

## Consequences

Default generated file output no longer promises deterministic regex traversal order.

Users who need deterministic output can pass `--ordered` or use an order-sensitive option.

Unordered file generation reduces per-Match String allocation and avoids ordered chunk draining. Generation caps that require exact global accounting may use the ordered path until unordered accounting is implemented without weakening limit semantics.
