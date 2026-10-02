# Code Review Toolkit (`code-review`)

> **Developer skills, opinionated Rust linters, and Inspect AI evaluations for reviewing agent-generated code.**

[![CI](https://github.com/drmartinwimmer/review/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/review/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)

**`code-review`** provides an end-to-end ecosystem to evaluate, inspect, and enforce code quality standards on agent-generated code. It pairs modular developer skills with high-performance Rust CLI tools and an automated evaluation suite powered by [Inspect AI](https://inspect.ai-safety-institute.org.uk/) and the [Fence](https://github.com/fencesandbox/fence) sandbox.

---

> [!IMPORTANT]
> **Disclaimer**: This is a personal project and does not represent the author's current or past employer.

---

## Core Components

### 1. Rust CLI Toolkit (`code-review`)

- **`code-review check`**: Runs all relevant linters and formatters (`cargo fmt`, `cargo clippy`, and custom linters), aggregating findings into a normalized diagnostic report (terminal, JSON, or Markdown) with Jujutsu changed-file filtering (`--changed-only`).
- **`code-review configure-lints`**: Programmatically updates `Cargo.toml` using `toml_edit` to inject strict Clippy rules (Don't Panic, Don't Fail Silently, Memory Safety, Numerics, Suppression Bans) while preserving formatting and comments.
- **`code-review opinionated`**: AST static analysis engine (powered by `syn`) enforcing patterns beyond Clippy's scope (e.g. forbidding inline `mod` declarations in `main.rs`/`lib.rs`, enforcing free functions over dummy structs, requiring VCS/manifest-relative paths, forbidding raw `Result<T, String>`, and validating `#[expect]`/`#[allow]` comments).

### 2. Developer & Review Skills (`skills/`)

- **Feedback Distillation (`skills/distilling-feedback`)**: Analyzes past agent transcripts and course corrections to distill recurring mistakes into actionable review guidelines and linter rules.
- **Thematic Review Skills**: Modular skills dispatched to subagents to review specific dimensions in parallel:
  - `reviewing-spec-compliance`: Verifies code meets specifications and milestone scope.
  - `reviewing-rust-modularity`: Ensures single responsibility and separate submodule files.
  - `reviewing-rust-robustness`: Checks error handling, error enums, and absence of unwraps/panics.
  - `reviewing-rust-testing`: Enforces `#[gtest]`, `expect_that!`, `.or_fail()?`, and clean teardown.
  - `reviewing-rust-lint-hygiene`: Enforces zero warnings and strict clippy compliance.
  - `reviewing-containment-safety`: Validates sandbox boundaries and VCS root-relative path resolution.

### 3. Inspect AI & Fence Evaluation Suite (`evals/`)

- Python-based evaluation suite managed with `uv` (`inspect-ai`).
- Uses `fence` to sandbox the agent under test (e.g. `agy`) with network restrictions and filesystem containment.
- Benchmarks agent review capabilities against minimal, abstracted code examples extracted from real past feedback.

---

## Development

This repository uses [Nix](https://nixos.org/) flakes, [Direnv](https://direnv.net/), and [Jujutsu (`jj`)](https://github.com/martinvonz/jj):

```bash
# Allow direnv to load development environment
direnv allow

# Check code formatting & lints
cargo fmt --check
cargo clippy --all-targets --all-features

# Run tests
cargo test --all-targets --all-features

# Build with Nix
nix build
```

---

## License

This project is licensed under the [MIT License](LICENSE).
