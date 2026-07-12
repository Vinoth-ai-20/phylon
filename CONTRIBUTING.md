# Contributing to Phylon

## Building and Testing

Phylon relies on `cargo` and requires a stable Rust toolchain (1.80+).
Compile the workspace locally with optimizations enabled:

```bash
cargo build --release
```

Run the test suite:

```bash
cargo test --all
```

`cargo-nextest` also works if you have it installed, for faster concurrent execution.

Before opening a pull request, run the same checks CI enforces:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo doc --no-deps --document-private-items
```

A pull request that doesn't pass all four cleanly will not merge.

For changes that touch viewport interaction, the camera, gizmos, or window
performance, also run through [MANUAL_TESTING.md](MANUAL_TESTING.md) — there
is no automated input-injection tooling for the live `winit`/`egui` window in
this project's environments, so that checklist is the actual interactive
verification for those areas.

## Branching Strategy

All pull requests must originate from feature branches using the following naming conventions:

- `feat/`: For new components, subsystems, or capabilities.
- `fix/`: For bug fixes and regressions.
- `docs/`: For documentation or comment updates.

## Architectural Enforcement

See [`docs/architecture/ARCHITECTURE_PRINCIPLES.md`](docs/architecture/ARCHITECTURE_PRINCIPLES.md) for the durable rules behind this section, and the six-question checklist to run any nontrivial feature or architectural proposal through before committing to it.

New crates and dependencies must preserve the workspace's acyclic dependency graph — see [`docs/reference/crate_graph.md`](docs/reference/crate_graph.md) for the current structure and boundaries. `app` is the only crate permitted to depend on everything else.

Any change that affects simulation outcomes must go through `common::SimRng`, the project's single seeded source of randomness — never an unseeded RNG (e.g. `fastrand::` used directly). See [`docs/explanation/determinism.md`](docs/explanation/determinism.md) for exactly what determinism guarantee this project makes today, and what's still an open gap — don't claim a PR "preserves determinism" without checking that document first.

For the architectural decisions a change might touch, see [`docs/roadmap/decisions.md`](docs/roadmap/decisions.md); for what's already known to be open/unfinished, see [`docs/roadmap/backlog.md`](docs/roadmap/backlog.md) before assuming something is a new bug.

## Licensing

By contributing to Phylon, you agree that your contributions will be dual-licensed under the MIT License and the Apache License, Version 2.0.
