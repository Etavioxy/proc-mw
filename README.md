# proc-mw

English | [中文](README.zh.md)

Production Service semantics × runtime hot-reload Rust middleware layer — eight core constraints continuously validated in a single crate.

## What this is

`proc-mw` is a Rust middleware system: the business core (pure functions) is decoupled from cross-cutting logic (middleware), which hooks in at core entry/exit, supports dynamic add/remove at runtime, and pursues **zero cost in Release** — full dynamics in Debug, and in Release the middleware layer is stripped away, degrading to bare function calls.

The design is judged by "eventually embedding in large Rust systems", with eight constraints continuously validated in one project:

| Dimension | Content | Landing point |
|---|---|---|
| D1 | Expression layer · zero-cost abstraction | `src/lib.rs` (Proc trait + shadowing + cfg assembly) |
| D2 | Typed channels · zero-cost dispatch | `src/dispatch.rs` (enum / fn pointer / bitflags / dyn / OpaqueNode) |
| D3 | Dynamicity · data/snapshot add-remove | `src/chain.rs` (RCU snapshot, lock-free zero-allocation read path) |
| D4 | Performance · local addition + transparent empty chain | `examples/d4_bench.rs` |
| D5 | Compile layer · LLVM structure + real dynamic linking | `examples/` + `labs/` |
| D6 | Extension form · runtime loading | `labs/` |
| D7 | Safety and diagnosis | `labs/` |
| D8 | Migration toolchain | `labs/d8_migrate/` |

**Core purpose**: `OpaqueNode` erases types with `*mut c_void` — the host and runtime-compiled plugins each define the same `#[repr(C)]` shared type layout; plugins call methods on arbitrary types internally, achieving true type openness + runtime hot reload.

## Documentation

- [`CORE-CONSTRAINTS.md`](CORE-CONSTRAINTS.md) — the eight core constraints, finalized (condensed)
- [`DESIGN-DRAFT.md`](DESIGN-DRAFT.md) — design exploration draft (with correction trail)
- [`docs/`](docs/) — per-dimension deep analysis, limit evaluation, and measured results

## How to explore

- **Examples**: the `d1`~`d7` series under `examples/` covers runnable validation of each dimension
- **Lab bench**: `labs/` is the measurement bench — synthetic code generation, standalone dylib compilation units, migration toolchains, and other isolated experiments
- **Tests**: `cargo test` runs the full suite (typed channels, sandbox rejection paths, timeout boundaries, etc.)

```bash
cargo build --release
cargo test
cargo run --example d4_bench --release   # performance dimension measurement
```

## AI-generated

> This codebase was written with AI assistance — 100% AI-generated (with human review). It is evidence for quality-assured engineering with AI tooling.

## License

[MIT](LICENSE)
