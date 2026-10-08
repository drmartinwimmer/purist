# Purist

> **A fast, opinionated AST-based static analysis tool and linter for enforcing strict Rust code hygiene, idiomatic patterns, and clean architecture.**

[![CI](https://github.com/drmartinwimmer/purist/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/purist/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/purist.svg)](https://crates.io/crates/purist)
[![Docs.rs](https://docs.rs/purist/badge.svg)](https://docs.rs/purist)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/drmartinwimmer/purist/blob/main/LICENSE)
[![Built with Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![MSRV: 1.88+](https://img.shields.io/badge/MSRV-1.88%2B-blue.svg?logo=rust)](https://www.rust-lang.org/)

## Why Purist? (In Addition to Clippy)

Purist is designed to be used **in addition to [Clippy](https://github.com/rust-lang/rust-clippy)**, not as a replacement:

- **Clippy** is the official compiler-integrated linter focused on type-aware correctness, performance optimizations, memory safety, and standard Rust idioms.
- **Purist** is an opinionated, AST-level static analyzer focused on **architectural boundaries, structural hygiene, interface design, and clean engineering practices**.

### Value Added Over Clippy

1. **Architectural & Design Boundaries**: Clippy does not enforce high-level codebase architecture. Purist enforces strict design discipline:
   - CLI encapsulation (`clap_struct_encapsulation`, `cli_run_consumes_self`).
   - Centralized external process execution (`centralized_command_execution`).
   - Clean module organization by preventing monolithic inlined submodules (`no_inline_mods`).
   - Preventing dummy empty structs used merely for function namespacing (`free_functions`).
   - Eliminating unnecessary getter/setter boilerplate (YAGNI; `no_trivial_getters_setters`).

2. **Configuration & Boundary Isolation**:
   - Forbids unconstrained `std::env` reads scattered across library or business logic, requiring environment access to be centralized in dedicated configuration modules (`no_env_access_outside_config`).
   - Enforces anchored filesystem paths instead of raw relative literals (`path_resolution`).

3. **Strict Error Handling & Process Hygiene**:
   - Forbids returning unstructured `String` or `&str` errors (`error_types`).
   - Banned usage of `Box<dyn Error>` in production code in favor of structured error enums (`no_boxed_dyn_error`).
   - Prohibits `std::process::exit` deep inside library modules (`exit_code_hygiene`).

4. **Codebase Hygiene & Maintainability**:
   - Requires every `#[allow(...)]` or `#[expect(...)]` compiler attribute to include an explicit, documented `reason` (`clippy_suppression_hygiene`).
   - Prohibits `println!` in library code (`no_println_in_libraries`).
   - Prohibits wildcard imports outside tests and preludes (`no_wildcard_imports`).
   - Prevents confusing double-negative boolean naming (`no_double_negation`).

5. **Testing Hygiene & Consistency**:
   - Standardizes test naming (`<verb>_<description>_<outcome>`) and GoogleTest matchers (`test_patterns`, `googletest_conventions`).
   - Eliminates redundant `test_` prefixes (`no_test_prefix`).
   - Bans raw `unsafe` blocks in tests without explicit suppression (`no_unsafe_in_tests`).
   - Mandates RAII temporary directory guards over manual removals (`raii_temp_directories`).

6. **Blazing Fast AST-Only Speed**:
   - Purist operates directly on the syntax tree via `syn` without invoking full compiler type-checking or cargo metadata resolution.
   - It runs in milliseconds, making it ideal for instant pre-commit hooks and local editor feedback.

### Guardrails for AI Coding Agents: Readable & Well-Structured Code

Modern AI coding agents (such as Claude Code, GitHub Copilot, Codex, and Cursor) generate syntactically valid Rust with ease, but frequently introduce architectural sprawl, overengineering, and subtle anti-patterns that degrade long-term maintainability.

Purist acts as an **automated, deterministic quality guardrail** that steers AI agents toward producing readable, idiomatic, and cleanly structured code:

- **Prevents Monolithic Sprawl & Inline Bloat**: Agents often drop entire submodule implementations directly into `main.rs` or `lib.rs`. Purist's `no_inline_mods` and `max_file_lines` rules require agents to decompose implementations into small, focused, dedicated files with clean module facades.
- **Eliminates AI Boilerplate & Overengineering**: LLMs often generate OOP-style habits from other languages, such as empty structs acting as dummy namespaces (`free_functions`), redundant getter/setter combos on simple structs (`no_trivial_getters_setters`), or wrapper functions that merely forward arguments without added value (`no_redundant_wrappers`). Purist keeps agent output clean, lean, and YAGNI-aligned.
- **Enforces Architectural Boundaries**: Agents often scatter `std::process::Command` calls or direct `std::env::var` reads throughout business logic. Purist mandates centralized command execution runners (`centralized_command_execution`) and dedicated configuration modules (`no_env_access_outside_config`).
- **Eliminates Code Smells & "AI Slop"**: Purist prevents common agent bad habits like inserting debug `println!` statements in libraries (`no_println_in_libraries`), generating confusing double-negative booleans like `with_skip_*` (`no_double_negation`), or spraying unreasoned `#[allow(...)]` attributes to silence compiler warnings (`clippy_suppression_hygiene`).
- **Forces Robust Error Handling**: Instead of letting agents fall back to sloppy `String` or `Box<dyn Error>` return types, Purist forces the definition of structured, typed error enums (`error_types`, `no_boxed_dyn_error`).
- **Standardizes Test Architecture**: Agents frequently write tests with redundant `test_` prefixes, unprincipled test names, or manual filesystem cleanups that leak on failure. Purist enforces structured `<verb>_<description>_<outcome>` test names, GoogleTest assertions, and RAII directory guards (`raii_temp_directories`).
- **Instant Agent Self-Correction Loop**: Because Purist is purely AST-based, it runs in milliseconds without waiting on cargo compilation. In autonomous agent loops (e.g., pre-commit hooks or tool invocations), agents get instantaneous feedback to self-correct architectural and style violations before presenting changes for review.

## Installation

```bash
cargo install purist
```

## Quick Start

Run Purist against your repository or workspace:

```bash
purist
```

By default, Purist analyzes the Cargo project in the current working directory (`.`), which must contain a `Cargo.toml`. Discovery is strictly focused on targets defined by the Cargo manifest (including all workspace members and child crates) with no directory-crawling fallbacks.

To analyze a specific file within the workspace, use the `--path` option:

```bash
purist --path src/lib.rs
```

To output results as JSON:

```bash
purist --format json
```

## Adopting Purist on an Existing Codebase

Adopting a strict, opinionated linter on an existing codebase can feel intimidating if dozens of violations fail the build all at once. Purist provides a seamless adoption path using the `--allow` flag.

### Step 1: Baseline Existing Violations

Run Purist once with the `--allow` flag:

```bash
purist --allow
```

**What this does**:

- Analyzes your entire codebase and identifies all rules that currently trigger violations.
- Automatically writes or updates the `[lints.purist]` table in `Cargo.toml` (or `[workspace.lints.purist]` in a virtual workspace root), setting every failing rule to `"allow"`.
- Rules that your existing code already satisfies remain enforced at their default level (`deny` or `warn`).

For example, your `Cargo.toml` will be updated with:

```toml
[lints.purist]
error_types = "allow"
no_inline_mods = "allow"
use_declarations_over_qualified_paths = "allow"
```

### Step 2: Lock In the Baseline in CI

With existing violations baseline-allowed in `Cargo.toml`, Purist will now pass cleanly:

```bash
purist
```

You can immediately add `purist` to your CI pipeline or pre-commit hooks. This prevents any **new** violations from creeping into compliant areas of the codebase without blocking ongoing development.

### Step 3: Re-Enable Rules One by One

When you are ready to refactor and raise the quality bar, tackle rules incrementally:

1. **Pick a rule**: Choose one rule in `Cargo.toml` to re-enable. Either delete the entry (to restore its default enforcement) or change its level to `"deny"`:
   ```toml
   [lints.purist]
   error_types = "deny" # Re-enabled!
   no_inline_mods = "allow"
   use_declarations_over_qualified_paths = "allow"
   ```
2. **Run Purist**: Run `purist` to see the localized diagnostics for that single rule.
3. **Refactor**: Clean up the offending code to comply with the rule.
4. **Verify**: Ensure `purist` exits with code 0.
5. **Commit**: Save your changes and repeat for the next allowed rule.

Over time, you can systematically remove all `"allow"` entries until your codebase is fully compliant.

## Rules Enforced

Purist enforces strict best practices across multiple areas of code quality:

- **Encapsulation & Boundaries**:
  - `purist::clap_struct_encapsulation`: CLI command structs should keep fields private and expose a `run(self)` execution method.
  - `purist::cli_run_consumes_self`: CLI execution methods consume `self` by value to avoid redundant cloning.
  - `purist::centralized_command_execution`: External command invocations must be centralized via dedicated runners.
  - `purist::no_inline_mods`: Modules should be placed in dedicated files, not inlined in root source files.
  - `purist::free_functions`: Functions should not be namespaced in dummy empty structs.
  - `purist::no_trivial_getters_setters`: Flags trivial getter/setter combos where exposing or accessing the field directly would suffice (YAGNI).
- **Hygiene & Readability**:
  - `purist::use_declarations_over_qualified_paths`: Disallows long inline qualified paths (`crate::a::b::C`) in favor of clear `use` imports.
  - `purist::no_wildcard_imports`: Wildcard imports (`use foo::*`) are prohibited outside test contexts and preludes.
  - `purist::no_redundant_wrappers`: Functions that merely forward arguments without additional logic are flagged.
  - `purist::no_println_in_libraries`: Libraries must use structured returns, diagnostic collectors, or logging facades rather than raw `println!`.
  - `purist::no_double_negation`: Prohibits negative boolean naming (such as `with_skip_*`, `is_skip_*`, `skip: bool`) to prevent double negations.
  - `purist::clippy_suppression_hygiene`: Every `#[allow(...)]` or `#[expect(...)]` attribute requires a documented `reason`.
- **Idiomatic Patterns**:
  - `purist::single_match_to_let_else`: Recommends `let ... = ... else { ... };` over single-variant `match` with early exits.
  - `purist::idiomatic_option_bool_mapping`: Prefers combinators like `.is_some_and(...)` over manual `if let Some(...) = ... else { false }`.
  - `purist::no_redundant_conversions`: Flags redundant sequential serialize/deserialize roundtrips.
- **Robustness & Error Handling**:
  - `purist::error_types`: Functions must not return raw `String` or `&str` error types.
  - `purist::no_boxed_dyn_error`: Production code must use structured error types instead of `Box<dyn Error>`.
  - `purist::exit_code_hygiene`: Direct `std::process::exit(...)` is forbidden in library modules.
  - `purist::path_resolution`: Unanchored relative path literals are prohibited in filesystem calls.
  - `purist::no_env_access_outside_config`: Direct `std::env` reads are restricted to designated config modules.
- **Testing Hygiene**:
  - `purist::test_patterns`: Standardizes test naming (`<verb>_<description>_<outcome>`), GoogleTest matchers, and error propagation (`?`) over `.unwrap()`.
  - `purist::no_test_prefix`: Forbids redundant `test_` prefixes in test function names.
  - `purist::no_unsafe_in_tests`: Forbids raw `unsafe` blocks and functions in test suites without explicit suppression.
  - `purist::raii_temp_directories`: Requires RAII temporary directory guards over manual `fs::remove_dir_all`.
  - `purist::test_matcher_borrow_simplification`: Disallows redundant `.as_str()` / `.as_slice()` in GoogleTest assertions.
  - `purist::googletest_conventions`: Enforces idiomatic GoogleTest conventions (`#[googletest::test]`, `assert_that!`, `?` over `.expect()`, and direct expressive matchers).

## License

MIT
