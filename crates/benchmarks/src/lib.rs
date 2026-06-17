//! # Phylon Benchmarks
//!
//! criterion benchmark harness for Phylon subsystems.
//!
//! Benchmarks live under `benches/` and are run with:
//!
//! ```bash
//! cargo bench
//! ```
//!
//! Results are published as HTML reports in `target/criterion/`.

#![warn(missing_docs)]
#![warn(clippy::all)]

// This crate is a benchmark harness only — it contains no public API.
// All benchmark logic lives in the `benches/` directory.
