# Global System Specification: Code Review Toolkit (`code-review`)

## 1. System Overview

### 1.1 Context and Mission
The `code-review` project provides an end-to-end quality and compliance review ecosystem designed specifically to evaluate, inspect, and enforce strict code quality standards on agent-generated code. As autonomous and semi-autonomous coding agents generate non-trivial codebases, they frequently introduce subtle anti-patterns: silent error swallowing, hidden panics (`unwrap`), unsafe slicing, architectural bloat, unauthorized public API creep, and missing test coverage.

The `code-review` ecosystem systematically catches and prevents these issues across four coordinated pillars:
1. **Rust CLI Toolkit (`code-review` crate):** High-performance static analysis tools, linter aggregators, AST inspectors, and manifest generators.
2. **Automated SemVer & Release Pipeline:** Intent-based releases governed by Conventional Commits (`Release Please`) cross-verified against code reality via `cargo-semver-checks`.
3. **Thematic Developer Review Skills (`skills/`):** Specialized agent skills dispatched to domain-focused subagents for parallel code review.
4. **Inspect AI Evaluation Suite (`evals/`):** Evaluation benchmarks executed in the `fence` sandbox against historical failure modes to continuously measure and improve agent review capabilities.

### 1.2 Target Architecture

```
+-------------------------------------------------------------------------+
|                              Developer / CI                             |
+------------------------------------+------------------------------------+
                                     |
                                     v
+-------------------------------------------------------------------------+
|                     Rust CLI Toolkit (`code-review`)                    |
|                                                                         |
|  * `check`: Aggregates formatters, clippy, opinionated, audit, coverage |
|  * `configure-lints`: Non-destructive Cargo.toml clippy injector        |
|  * `opinionated`: Syn AST engine checking patterns beyond Clippy        |
|  * `api`: Introspects and detects API drift against API.md              |
|  * `coverage`: LLVM source-based coverage gates                         |
+------------------------------------+------------------------------------+
                                     |
         +---------------------------+---------------------------+
         |                                                       |
         v                                                       v
+----------------------------------+   +----------------------------------+
|   Automated SemVer Verification  |   |   Modular Developer Skills       |
|                                  |   |                                  |
| * Release Please (Commit Intent) |   | * Distilling Feedback            |
| * cargo-semver-checks (Reality)  |   | * Specialized Subagent Reviews   |
| * Checked-in API.md Manifest     |   | * Sandboxed Inspect AI Evals     |
+----------------------------------+   +----------------------------------+
```

---

## 2. System Invariants

These architectural rules are absolute guarantees that must never be broken by any component of the system.

### Invariant 1: Standardized CLI Conventions and Exit Codes
All CLI subcommands must follow Unix stream conventions and standard exit code semantics:
- **`stdout`:** Dedicated strictly to primary command output, structured machine-readable payloads (JSON), or human-readable reports.
- **`stderr`:** Dedicated strictly to logs, informational progress messages, warnings, and diagnostic errors.
- **Exit Codes:**
  - `0 (Success)`: Operation completed cleanly, or all static checks / linters reported zero errors and zero unsuppressed warnings.
  - `1 (Lint / Check Failure)`: Tool executed successfully, but one or more static checks, lint rules, API drift validations, or coverage gates failed.
  - `2 (Execution / Runtime Error)`: Tool execution failed due to invalid arguments, missing files, I/O errors, or external toolchain invocation failure.

### Invariant 2: Non-Destructive and Idempotent Configuration Modification
Any automated modification to user configuration files (specifically `Cargo.toml`) must:
- Use concrete syntax tree (CST) preserving parsers (specifically `toml_edit`).
- Preserve all existing comments, whitespace, empty lines, and table ordering verbatim.
- Preserve all existing tables, arrays, and keys not targeted by the modification.
- Be strictly **idempotent**: running the command multiple times consecutively on the same file produces byte-for-byte identical content and does not generate redundant diffs.

### Invariant 3: Jujutsu-Native Version Control
All version control operations within tooling, scripts, CI, and agent execution must interact exclusively with Jujutsu (`jj`):
- All command executions must use Jujutsu (`jj`) with the global `--no-pager` flag (e.g. `jj --no-pager status`, `jj --no-pager diff`).
- Direct Git commands (e.g., `git commit`, `git add`, `git checkout`) and Git-isms in Jujutsu (e.g., `jj branch`) are forbidden.
- Working copy cleanliness and structured description prefixes (`<slug>-M<milestone>-T<task>:`) are required for all changes.

### Invariant 4: Robust, Structured Error Handling
All error handling across the codebase must adhere to strict typing:
- Internal libraries, tools, and shared modules must use structured error enums derived with `thiserror`.
- Only top-level CLI entry points (`main.rs`) may use `anyhow::Result<()>` to report user-facing error chains and set exit codes.
- **Absolute Ban on Panics:** Production code must never use `unwrap()`, `expect()`, `panic!()`, `todo!()`, or `unimplemented!()`.
- **Absolute Ban on Raw String Errors:** Functions must never return `Result<T, String>` or `Result<T, &'static str>`.

### Invariant 5: Minimal and Audited Public API Surfaces
- Crate modules and internal helpers must default to `pub(crate)` visibility.
- Items are made `pub` only when explicitly intended as an external crate contract.
- Any change to public library items, CLI command trees, or service endpoints must be reflected in the checked-in `API.md` manifest and validated by SemVer tooling.

### Invariant 6: Workspace-Relative Containment
- All file access, path arguments, and linter targets must be resolved relative to the detected repository or workspace root.
- Tools must never escape workspace boundaries or access arbitrary host paths without explicit configuration.

---

## 3. Core Concepts and Ubiquitous Language

| Term | Definition |
| :--- | :--- |
| **`Diagnostic`** | An individual quality, safety, or style finding produced by a tool, containing a file path, span (line/column/offset), severity, rule identifier, explanation message, and optional structured fix suggestion. |
| **`DiagnosticReport`** | An aggregated container of all diagnostics produced during a check run, summarizing total errors, warnings, scanned targets, and duration. |
| **`Severity`** | Classification level of a diagnostic: `Error` (blocks CI / exit code 1), `Warning` (reported, blocks under `--deny-warnings`), `Info`, or `Hint`. |
| **`Span`** | Source code location denoting start and end line/column coordinates and byte offsets. |
| **`Opinionated Lint`** | A static analysis rule implemented using AST inspection (`syn`) targeting architectural anti-patterns that standard Clippy rules cannot enforce. |
| **`AstVisitor`** | Traversal framework that visits Rust syntax tree nodes (items, functions, modules, expressions) without invoking the Rust compiler. |
| **`API Manifest`** | A checked-in source-of-truth document (`API.md`) recording all public library symbols, CLI commands and flags, and HTTP routes to detect unintentional modifications. |
| **`Drift`** | Uncommitted or unapproved divergence between the actual code surface and the checked-in `API.md` manifest. |
| **`Deprecation`** | The systematic decoration of phased-out APIs with `#[deprecated]`, warning downstream users for at least one minor/major release before removal. |
| **`Conventional Commits`** | Commit message specification (`feat:`, `fix:`, `feat!:`, `BREAKING CHANGE:`) expressing developer intent for version bumps. |
| **`Release Please`** | Automation system that reads Conventional Commits to maintain changelogs and prepare release pull requests. |
| **`SemVer Check`** | Mechanized validation via `cargo-semver-checks` analyzing compiled `rustdoc` JSON to guarantee that code changes legally match the SemVer version bump. |

---

## 4. Cross-Cutting Concerns

### 4.1 Output Formats
Tools that emit diagnostic reports must support multiple output formats selectable via `--format <FORMAT>`:
- **`console` (default):** Colorized terminal output using ANSI formatting, grouping diagnostics by file with clear line and column pointers.
- **`json`:** Machine-readable JSON output matching `DiagnosticReport` serialization for programmatic consumption by CI bots and agent tools.
- **`markdown`:** Structured GitHub-flavored markdown with collapsible summary tables, suitable for PR comments and review artifacts.

### 4.2 Logging and Verbosity
- Standard logging must be managed via the `tracing` or `env_logger` facade.
- Subcommands must accept `-v` / `--verbose` and `-q` / `--quiet` flags.
- Logs and progress indicators must always be directed to `stderr`, leaving `stdout` free for structured data.

### 4.3 Workspace and VCS Detection
- Tools must automatically discover the repository root by locating `.jj` or `.git` directories, or the root `Cargo.toml`.
- When operating in a Cargo workspace, tools must support workspace-wide operations or individual member targeting via `--package <NAME>`.
- The `--changed-only` flag queries Jujutsu (`jj --no-pager diff --name-only`) to restrict linting and AST checks to files modified in the active revision.

### 4.4 Performance Budgets & Subprocess Timeouts
- **Subprocess Timeouts:** External toolchain invocations (e.g., `cargo clippy`, `cargo fmt`, `cargo audit`, `cargo-llvm-cov`, `cargo-semver-checks`, `jj`) must enforce a strict default timeout of 60 seconds per command to prevent hung child processes or deadlocks.
- **Graceful Termination:** Processes exceeding the timeout must be cleanly killed (SIGTERM followed by SIGKILL if unresponsive) and reported as an execution runtime error (Exit Code 2).
- **Execution Efficiency:** Avoid unnecessary full-workspace re-compilations where AST-only static analysis (`syn`) or changed-file filtering (`--changed-only`) suffices.

