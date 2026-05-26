//! Library entry point.
//!
//! The same modules used by the `openapi_parser` binary are exposed here as
//! a regular Rust crate so that integration tests (in `tests/`) and other
//! callers can drive the parser and code generator directly without
//! invoking the CLI.
//!
//! The companion binary in `src/main.rs` re-uses everything from here via
//! `use openapi_parser::...`.

pub mod generate;
pub mod parse;

use generate::{DartGenerator, File, GenerationArgs, Generator};

/// Run the full pipeline (`spec JSON` -> `IntermediateFormat` -> generated
/// Dart files) and return the list of files that would be written to disk.
///
/// Intended for use from tests and tooling; the CLI in `main.rs` performs
/// the same steps inline so that it can also stream progress logs.
///
/// `ignore_deprecated_fields` is forwarded as-is to
/// [`GenerationArgs::ignore_deprecated_fields`]; pass `true` to filter out
/// properties/parameters that reference deprecated schemes, `false` to
/// keep them.
pub async fn generate_dart_files(
    spec_json: &str,
    ignore_deprecated_fields: bool,
) -> Result<Vec<File>, String> {
    let spec = oas3::from_json(spec_json).map_err(|e| format!("parse spec: {:?}", e))?;
    DartGenerator
        .generate(
            &spec,
            GenerationArgs {
                ignore_deprecated_fields,
            },
        )
        .await
}
