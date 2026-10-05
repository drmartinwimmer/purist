# Module Specification: Check Aggregator (`code-review-check`)

## 1. Purpose and Overview

The `code-review check` command is the central quality and conformance aggregator of the `code-review` toolkit. It coordinates the execution of static analysis tools, linters, formatters, and security scanners across a Cargo project or workspace, synthesizing their output into a unified diagnostic model (`purist::DiagnosticReport`).

The aggregator executes:

1. **Formatting Check (`cargo fmt --check`)**: Verifies that all Rust source code conforms to the standard rustfmt configuration.
2. **Clippy & Compiler Lints (`cargo clippy --message-format=json`)**: Runs compiler and Clippy static analysis with warnings/errors streamed as structured JSON.
3. **Purist AST Rules (`code-review purist`)**: Runs custom AST-based architecture, style, and hygiene rules.
4. **Security Vulnerability Audit (`cargo audit --json`)**: Scans dependencies in `Cargo.lock` against the RustSec Advisory Database.
5. **Markdown Format & Lint Check (`prettier --check` / `mdformat --check`)**: If available in the environment, verifies formatting and syntax of `*.md` files.
6. **TOML Format & Lint Check (`taplo fmt --check` / in-process `toml_edit`)**: If available in the environment, verifies formatting and syntax of `*.toml` files.
7. **JSON Format & Lint Check (`prettier --check` / in-process `serde_json`)**: If available in the environment, verifies formatting and syntax of `*.json` files.
8. **Jujutsu Changed-File Filtering (`--changed-only`)**: When enabled, queries `jj --no-pager diff --summary` to scope reported diagnostics exclusively to files modified or added in the active working revision.

---

## 2. Invariants and Architectural Guarantees

1. **Normalized Diagnostic Stream**: Regardless of the underlying tool's native output format, all findings must be parsed and represented as `purist::Diagnostic` items containing a standardized rule identifier, severity (`Error`, `Warning`, `Info`, `Hint`), source code span (`Span`), message, and optional suggested fix.
2. **Centralized Process Execution**: In compliance with `purist::centralized_command_execution`, all external process invocations (`std::process::Command::new`) must be isolated within dedicated tool runner modules under `crates/check/src/tools/`.
3. **Resilience & Graceful Degradation ("If Available")**:
   - Optional external checkers/formatters (`prettier`, `mdformat`, `taplo`, `cargo-audit`) probe availability before running.
   - If an optional tool is missing from the environment, it is skipped cleanly or backed by in-process syntax verification without failing the suite.
   - If `cargo-audit` fails to acquire the network lock, it automatically retries with `--no-fetch`.
4. **Deterministic Exit Codes**:
   - Exit code `0`: Check execution succeeded and no violations exceeded the configured `--fail-on` threshold.
   - Exit code `1`: Quality, style, or security violations were detected that meet or exceed the `--fail-on` threshold.
   - Exit code `2`: Operational error (e.g., target directory not found, invalid command-line arguments, or VCS failure when `--changed-only` is requested).
5. **Format Transparency**: Supports all standard diagnostic output formats (`console`, `json`, `markdown`), delegating formatting to `purist::render_report`.
6. **No Library Output Side Effects**: In compliance with `purist::no_println_in_libraries`, library functions must not emit unbuffered `println!` or `eprintln!` directly; execution produces a `DiagnosticReport` or writes to an injected `&mut dyn std::io::Write`.

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
      --skip-purist              Skip running purist AST linter (alias: --skip-opinionated)
      --skip-audit               Skip running cargo audit
      --skip-markdown            Skip running markdown format/lint checks
      --skip-toml                Skip running TOML format/lint checks
      --skip-json                Skip running JSON format/lint checks
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

### 4.4 Purist AST Runner (`tools/purist.rs`)

- **Execution**: Invokes `purist::PuristEngine::check_path` directly in-process.
- **Diagnostics**: Directly aggregates all findings from the custom AST-based architecture, style, and hygiene rules.
- **Compatibility**: Aliased as `OpinionatedRunner` for backwards compatibility.

### 4.5 Markdown Runner (`tools/markdown.rs`)

- **Execution**: Probes for `prettier --check` (or `mdformat --check`). If unavailable, skips cleanly.
- **Output Parsing**:
  - Parses formatting warnings into `rule: "fmt::markdown"`, `severity: Severity::Warning`, and suggested write command.
  - Parses syntax errors into `rule: "fmt::markdown"`, `severity: Severity::Error`.

### 4.6 TOML Runner (`tools/toml.rs`)

- **Execution**: Probes for `taplo fmt --check`. If unavailable, executes in-process syntax validation via `toml_edit`.
- **Output Parsing**:
  - Formatting violations mapped to `rule: "fmt::toml"`, `severity: Severity::Warning`.
  - Syntax errors mapped to `rule: "toml::syntax"`, `severity: Severity::Error`.

### 4.7 JSON Runner (`tools/json.rs`)

- **Execution**: Probes for `prettier --check`. If unavailable, executes in-process syntax validation via `serde_json`.
- **Output Parsing**:
  - Formatting violations mapped to `rule: "fmt::json"`, `severity: Severity::Warning`.
  - Syntax errors mapped to `rule: "json::syntax"`, `severity: Severity::Error`.

### 4.8 Jujutsu VCS Filter (`tools/vcs_jj.rs`)

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
    Vcs(#[from] JjError),

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
   - `purist` runner tests verifying clean code and lint violation detection.
   - `markdown`, `toml`, and `json` runner tests verifying parser behavior and in-process fallback.
   - `vcs_jj` parser tests with various diff summary formats (M, A, R, C, D).
   - Filter logic tests verifying `--changed-only` and `--fail-on`.
2. **Integration Tests**:
   - Running `CheckCommand` on a clean mock crate verifying exit code 0.
   - Running `CheckCommand` on a mock crate containing clippy/fmt violations verifying exit code 1.
   - CLI parsing tests verifying all flags and default options.
