# Contributing to Phylon

## Building and Testing

Phylon relies on `cargo` and requires a stable Rust toolchain.
Compile the workspace locally with optimizations enabled:

```bash
cargo build --release
```

Execute the test suite utilizing `cargo-nextest` for concurrent test execution (or standard `cargo test`):

```bash
cargo nextest run
```

## Branching Strategy

All pull requests must originate from feature branches using the following naming conventions:

- `feat/`: For new components, subsystems, or capabilities.
- `fix/`: For bug fixes and regressions.
- `docs/`: For documentation or comment updates.

## Architectural Enforcement

All simulation decisions, library additions, and architectural patterns must strictly conform to the specifications defined in `PHYLON_PROMPT_v2.md`. Pull requests that violate the deterministic boundary conditions, introduce circular dependencies, or fail to adhere to the defined crate structure will be rejected.
