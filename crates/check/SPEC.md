# Module Specification: Check Aggregator (`code-review-check`)

## 1. Purpose and Overview

The `code-review check` command is the central quality and conformance aggregator of the `code-review` toolkit. It coordinates the execution of static analysis tools, linters, formatters, and security scanners across a Cargo project or workspace, synthesizing their output into a unified diagnostic model (`code_review_diagnostics::DiagnosticReport`).

The aggregator executes:
1. **Formatting Check (`cargo fmt --check`)**: Verifies that all Rust source code conforms to the standard rustfmt configuration.
2. **Clippy & Compiler Lints (`cargo clippy --message-format=json`)**: Runs compiler and Clippy static analysis with warnings/errors streamed as structured JSON.
3. **Opinionated AST Rules (`code-review opinionated`)**: Runs custom AST-based architecture, style, and hygiene rules.
4. **Security Vulnerability Audit (`cargo audit --json`)**: Scans dependencies in `Cargo.lock` against the RustSec Advisory Database.
5. **Jujutsu Changed-File Filtering (`--changed-only`)**: When enabled, queries `jj --no-pager diff --summary` to scope reported diagnostics exclusively to files modified or added in the active working revision.

---

## 2. Invariants and Architectural Guarantees

1. **Normalized Diagnostic Stream**: Regardless of the underlying tool's native output format, all findings must be parsed and represented as `code_review_diagnostics::Diagnostic` items containing a standardized rule identifier, severity (`Error`, `Warning`, `Info`, `Hint`), source code span (`Span`), message, and optional suggested fix.
2. **Centralized Process Execution**: In compliance with `opinionated::centralized_command_execution`, all external process invocations (`std::process::Command::new`) must be isolated within dedicated tool runner modules under `crates/check/src/tools/`.
3. **Resilience & Graceful Degradation**:
   - If an optional tool such as `cargo-audit` is missing from the environment, the aggregator must record an informational diagnostic or warning instead of crashing the entire check suite.
   - If `cargo-audit` fails to acquire the network lock, it automatically retries with `--no-fetch`.
4. **Deterministic Exit Codes**:
   - Exit code `0`: Check execution succeeded and no violations exceeded the configured `--fail-on` threshold.
   - Exit code `1`: Quality, style, or security violations were detected that meet or exceed the `--fail-on` threshold.
   - Exit code `2`: Operational error (e.g., target directory not found, invalid command-line arguments, or VCS failure when `--changed-only` is requested).
5. **Format Transparency**: Supports all standard diagnostic output formats (`console`, `json`, `markdown`), delegating formatting to `code_review_diagnostics::render_report`.
6. **No Library Output Side Effects**: In compliance with `opinionated::no_println_in_libraries`, library functions must not emit unbuffered `println!` or `eprintln!` directly; execution produces a `DiagnosticReport` or writes to an injected `&mut dyn std::io::Write`.

---

## 3. Command Line Interface Specification

```
code-review check [OPTIONS]

Options:
      --path <PATH>              Path to target workspace or crate directory [default: .]
      --format <FORMAT>          Output format: console, json, markdown [default: console]
      --fail-on <LEVEL>          Severity threshold triggering non-zero exit: warnings, errors [default: warnings]
      --changed-only             Filter diagnostics to only files modified in Jujutsu working copy
      --skip-fmt                 Skip running cargo fmt
      --skip-clippy              Skip running cargo clippy
      --skip-opinionated         Skip running opinionated AST linter
      --skip-audit               Skip running cargo audit
  -q, --quiet                    Silence non-essential status messages
  -h, --help                     Print help
```

---

## 4. Subprocess Runners & Parsers

### 4.1 Rustfmt Runner (`tools/cargo_fmt.rs`)
- **Execution**: `cargo fmt --check [--manifest-path <path>]`
- **Output Parsing**:
  - Exit code `0`: No formatting violations.
  - Exit code `1`: Parse stderr/stdout for `Diff in <file> at line <line>:` patterns.
  - Generates diagnostics with `rule: "fmt::formatting"`, `severity: Severity::Warning`, and a suggested fix `"Run 'cargo fmt' to format this file"`.

### 4.2 Cargo Clippy Runner (`tools/cargo_clippy.rs`)
- **Execution**: `cargo clippy --all-targets --all-features --message-format=json`
- **Output Parsing**:
  - Reads line-delimited JSON objects.
  - Filters for messages where `reason == "compiler-message"`.
  - Extracts `code.code` as the diagnostic rule identifier (e.g., `clippy::unwrap_used`, `clippy::redundant_clone`).
  - Maps `level` (`"error"` -> `Severity::Error`, `"warning"` -> `Severity::Warning`, `"note"` -> `Severity::Info`, `"help"` -> `Severity::Hint`).
  - Extracts the primary span (`is_primary == true`) for file name and line/column coordinates.
  - Extracts suggested replacement code from primary span or child suggestions.

### 4.3 Cargo Audit Runner (`tools/cargo_audit.rs`)
- **Execution**: `cargo audit --json` (with automatic fallback to `cargo audit --no-fetch --json`)
- **Output Parsing**:
  - Parses JSON output object.
  - Iterates over `vulnerabilities.list`, mapping each advisory to `rule: "audit::advisory"`, `severity: Severity::Error`, description, and affected package.
  - Iterates over `warnings`, mapping unmaintained or unsound crate notices to `rule: "audit::warning"`, `severity: Severity::Warning`.
  - If the `cargo-audit` binary is not found, records an informational diagnostic.

### 4.4 Opinionated AST Runner (`tools/opinionated.rs`)
- **Execution**: Invokes `code_review_opinionated::OpinionatedEngine::check_path` directly in-process or via tool runner.
- **Diagnostics**: Directly aggregates all findings from the 23 opinionated static analysis rules.

### 4.5 Jujutsu VCS Filter (`tools/vcs_jj.rs`)
- **Execution**: `jj --no-pager diff --summary`
- **Parsing**:
  - Recognizes summary status codes:
    - `M <file>` (modified)
    - `A <file>` (added)
    - `C <old> -> <new>` (copied)
    - `R <old> -> <new>` (renamed)
  - Produces a set of modified canonicalized file paths.
  - Diagnostics targeting files outside this set are filtered out of the final report. Diagnostics with no associated file span are preserved.

---

## 5. Error Taxonomy

```rust
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    #[error("Target path '{0}' was not found")]
    PathNotFound(std::path::PathBuf),

    #[error("Jujutsu VCS error: {0}")]
    Vcs(String),

    #[error("Tool execution failed for '{tool}': {details}")]
    ToolExecution { tool: String, details: String },

    #[error("I/O error during check execution: {0}")]
    Io(#[from] std::io::Error),

    #[error("Check violations found ({count} issues exceed failure threshold)")]
    ViolationsFound { count: usize },
}
```

---

## 6. Testing Strategy

1. **Unit Tests**:
   - `cargo_fmt` parser tests with mock diff output.
   - `cargo_clippy` JSON stream parser tests with mock compiler messages (errors, warnings, spans, suggestions).
   - `cargo_audit` JSON parser tests with mock advisory and warning payloads.
   - `vcs_jj` parser tests with various diff summary formats (M, A, R, C, D).
   - Filter logic tests verifying `--changed-only` and `--fail-on`.
2. **Integration Tests**:
   - Running `CheckCommand` on a clean mock crate verifying exit code 0.
   - Running `CheckCommand` on a mock crate containing clippy/fmt violations verifying exit code 1.
   - CLI parsing tests verifying all flags and default options.
