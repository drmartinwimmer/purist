# code-review-check

> **Unified quality and conformance check aggregator for Rust codebases.**

`code-review-check` coordinates the execution of static analysis tools, linters, formatters, and security scanners across a Cargo project or workspace, synthesizing their output into a unified diagnostic model.

## Included Checks

1. **Rust formatting**: `cargo fmt --check`
2. **Rust compiler & Clippy lints**: `cargo clippy --message-format=json`
3. **Purist AST linter**: `purist` static analysis rules
4. **Dependency security audit**: `cargo audit --json`
5. **Markdown formatting & linting**: `prettier` / `mdformat` (if available)
6. **TOML formatting & linting**: `taplo` / `toml_edit` (if available)
7. **JSON formatting & linting**: `prettier` / `serde_json` (if available)
8. **Jujutsu changed-files scoping**: `--changed-only` via `jj --no-pager diff --summary`

## Installation

```bash
cargo install code-review-check --bin check
```

## Usage

```bash
# Run all checks across current directory
check

# Filter checks to only files modified in the active Jujutsu revision
check --changed-only

# Output report as JSON or Markdown
check --format json
check --format markdown
```
