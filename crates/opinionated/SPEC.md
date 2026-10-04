# Opinionated Static Analysis Linter Specification (`code-review-opinionated`)

## 1. Overview & Purpose

The `code-review-opinionated` crate provides an AST-based static analysis engine designed to flag architectural, stylistic, and behavioral anti-patterns in Rust code that standard Clippy intentionally avoids checking or cannot inspect at the AST level.

Standard Clippy focuses on localized code simplifications, performance improvements, and language pitfalls. `code-review-opinionated` enforces repository-wide architectural conventions:
- Ensuring modular source tree layout instead of bloated inline modules.
- Enforcing idiomatic Rust free functions over object-oriented dummy namespace structs.
- Preventing unanchored relative path operations that fail when invoked from arbitrary working directories.
- Enforcing typed error enums (`thiserror`) instead of raw `String` errors in library APIs.
- Requiring strict suppression hygiene (mandatory `reason` and preceding explanatory comments) for all lint suppressions.
- Validating test conventions (naming conventions, GoogleTest matchers over `assert_eq!`, error propagation over `.unwrap()`).
- Eliminating redundant serialization round-trips.

---

## 2. Architectural Invariants

1. **Non-Panicking AST Processing:**
   All files are parsed using `syn`. Syntax errors or I/O failures MUST NOT panic the engine; they are converted into normalized `Diagnostic` items with `Severity::Error` or handled gracefully.

2. **Zero Side-Effects:**
   Analysis is strictly read-only. File contents and metadata are never altered during scanning.

3. **Diagnostic Normalization:**
   All rule violations produce `code_review_diagnostics::Diagnostic` objects containing:
   - `rule`: Fully qualified rule name (e.g., `opinionated::no_inline_mods`).
   - `severity`: Standard severity level (`Error`, `Warning`, `Info`, `Hint`).
   - `message`: Clear explanation of the violation and why it is problematic.
   - `span`: Precise file, line, and column coordinates.
   - `suggested_fix`: Actionable guidance or remediation snippet.

4. **Directory & Target Containment:**
   Recursive directory scanning MUST automatically skip build artifacts and VCS directories:
   - `target/`
   - `.git/`
   - `.jj/`
   - `.direnv/`
   - Hidden directories (starting with `.`).

5. **Standard Exit Codes:**
   - `0`: Execution succeeded, no error-level diagnostics found.
   - `1`: One or more lint violations were detected.
   - `2`: Command-line or runtime execution error (e.g. invalid arguments, unreadable path).

---

## 3. Custom Lint Rules Specification

### Rule 1: `opinionated::no_inline_mods`
- **Goal:** Enforce modular file organization by disallowing inline `mod foo { ... }` blocks in entry point files (`main.rs`, `lib.rs`), requiring submodules to reside in dedicated files (`foo.rs` or `foo/mod.rs`).
- **AST Pattern:** `syn::Item::Mod` where `item_mod.content.is_some()`.
- **Target Files:** Root module entry points (`main.rs`, `lib.rs`, `bin/*.rs`).
- **False-Positive Mitigation:** Modules annotated with `#[cfg(test)]` (e.g., `#[cfg(test)] mod tests { ... }`) are explicitly exempted.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Extract module content into `<mod_name>.rs` or `<mod_name>/mod.rs`."

### Rule 2: `opinionated::free_functions`
- **Goal:** Flag OOP-style dummy unit structs used solely as static namespaces (e.g. `pub struct Parser; impl Parser { pub fn parse(...) }`) and encourage idiomatic free functions in the module namespace.
- **AST Pattern:**
  1. A struct definition (`syn::Item::Struct`) with unit or empty fields (`syn::Fields::Unit` or `syn::Fields::Named` with 0 fields).
  2. One or more `impl StructName` blocks (`syn::Item::Impl` where `trait_.is_none()`).
  3. Every associated function in the `impl` block has no receiver (`receiver` is `None`, i.e., no `self`, `&self`, or `&mut self`).
  4. The struct implements no traits.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Replace dummy namespace struct with free functions in the module."

### Rule 3: `opinionated::path_resolution`
- **Goal:** Prevent unanchored relative path operations that assume the current working directory matches the crate or workspace root.
- **AST Pattern:**
  - Invocations of:
    - `Path::new("...")`, `PathBuf::from("...")`
    - `std::fs::read*("...")`, `fs::read*("...")`
    - `std::fs::write*("...")`, `fs::write*("...")`
    - `File::open("...")`, `File::create("...")`
  - Where the first argument is a string literal containing a relative path (does not start with `/`, `\\`, and is not an empty string or URL).
- **False-Positive Mitigation:**
  - Paths combined with `env!("CARGO_MANIFEST_DIR")` or workspace discovery functions are allowed.
  - Paths inside unit/integration test files are exempted if explicitly scoped to scratch test directories.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Anchor path using `Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(...)` or a workspace-resolved path."

### Rule 4: `opinionated::error_types`
- **Goal:** Ensure library and production functions return structured error types rather than unstructured strings.
- **AST Pattern:**
  - `syn::FnDecl` / `syn::Signature::output` returning `Result<T, String>` or `Result<T, &str>`.
- **Target Files:** Non-test functions in production code.
- **False-Positive Mitigation:**
  - Functions annotated with `#[test]` or inside `#[cfg(test)]` modules are exempted.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Use a structured error enum deriving `thiserror::Error` or `anyhow::Result` instead of raw string errors."

### Rule 5: `opinionated::clippy_suppression_hygiene`
- **Goal:** Ensure all lint suppressions are deliberate, documented, and justified.
- **AST Pattern:**
  - Attributes matching `#[allow(...)]` or `#[expect(...)]`.
  - Violations occur if:
    1. The attribute does not include a `reason = "..."` key-value pair.
    2. The line immediately preceding the attribute does not contain an explanatory comment (`// ...`).
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Add `reason = \"...\"` to the suppression attribute and include an explanatory comment on the preceding line."

### Rule 6: `opinionated::test_patterns`
- **Goal:** Enforce consistent test structure, assertion matchers, and error propagation in test suites.
- **Target Scope:** Functions annotated with `#[test]` or `#[googletest::test]`, or inside `#[cfg(test)]` modules.
- **Checks:**
  1. **Test Function Naming:** Test names must follow `<verb>_<description>_<outcome>` (at least 3 snake_case segments, e.g. `parse_valid_manifest_succeeds`, `check_missing_file_returns_error`).
  2. **Assertion Macro:** Usage of `assert_eq!` or `assert_ne!` is flagged; GoogleTest matchers (`assert_that!`, `expect_that!`) are recommended.
  3. **Unwrap in Tests:** Calling `.unwrap()` on `Result` or `Option` inside test bodies is flagged; returning `googletest::Result<()>` and using `?` is recommended.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Rename test to `<verb>_<description>_<outcome>`, use GoogleTest `assert_that!`/`expect_that!`, and use `?` instead of `.unwrap()`."

### Rule 7: `opinionated::no_redundant_conversions`
- **Goal:** Detect and flag unnecessary serialization roundtrips (e.g. `serde_json::to_string` followed immediately by `serde_json::from_str`).
- **AST Pattern:** Expressions or statement blocks that serialize a data structure to a string/bytes representation and immediately parse it back into another data structure in the same scope.
- **False-Positive Mitigation:** Exclude test functions that explicitly test serialization/deserialization fidelity.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Use `Clone`, `From`/`Into`, or `serde_json::to_value` instead of stringifying and re-parsing."

### Rule 8: `opinionated::use_declarations_over_qualified_paths`
- **Goal:** Forbid verbose qualified namespace paths in function bodies and signatures, requiring clean `use` declarations at the top of the file.
- **AST Pattern:** `syn::TypePath` and `syn::ExprPath` with $\ge 3$ segments starting with `crate::` or `super::`, or $\ge 2$ segments starting with an external crate, outside of `use` statements.
- **False-Positive Mitigation:** Exempt standard library paths (`std::path::Path`, etc.), macro invocations, and disambiguation paths (`<Type as Trait>::method`).
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Import '<Type>' via 'use <path>::<Type>;' at the top of the module."

### Rule 9: `opinionated::no_redundant_wrappers`
- **Goal:** Flag thin free function wrappers that merely forward identical arguments to a struct method or another function.
- **AST Pattern:** `syn::ItemFn` whose body contains a single statement calling `Target::method(...)` or `receiver.method(...)` forwarding identical parameters without modification or error mapping.
- **False-Positive Mitigation:** Exempt trait implementations, deprecated functions, and functions performing argument type conversion.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Remove redundant wrapper function; call '<Target>::<method>' directly."

### Rule 10: `opinionated::no_boxed_dyn_error`
- **Goal:** Ban `Box<dyn std::error::Error>` in production code return types, enforcing typed domain errors deriving `thiserror::Error`.
- **AST Pattern:** `Result<_, Box<dyn Error>>` in `syn::Signature::output` of non-test functions.
- **False-Positive Mitigation:** Exempt tests and `fn main()` in binary entry points.
- **Severity:** `Severity::Error`.
- **Suggested Fix:** "Return a concrete domain error enum deriving 'thiserror::Error' with appropriate '#[from]' conversions."

### Rule 11: `opinionated::test_matcher_borrow_simplification`
- **Goal:** Eliminate redundant `.as_str()`, `.as_slice()`, or `.as_ref()` conversions inside GoogleTest assertion macros.
- **AST Pattern:** `syn::Macro` invocations of `assert_that!` or `expect_that!` calling `.as_str()`, `.as_slice()`, or `.as_ref()` on tested expressions or expected values.
- **False-Positive Mitigation:** Only flagged in GoogleTest matcher assertion contexts where deref coercion works with `&`.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Replace '.<method>()' with a simple borrow '&<expr>'."

### Rule 12: `opinionated::no_test_prefix`
- **Goal:** Forbid prefixing test function names with `test_` or `test`, enforcing descriptive `<action>_<scenario>_<outcome>` naming.
- **AST Pattern:** `syn::ItemFn` annotated with `#[test]` or `#[googletest::test]` whose identifier starts with `test_` or `test`.
- **False-Positive Mitigation:** None; all tests should be named descriptively.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Remove redundant 'test_' prefix; use '<action>_<scenario>_<outcome>' (e.g. 'parse_valid_manifest_succeeds')."

### Rule 13: `opinionated::no_unsafe_in_tests`
- **Goal:** Forbid `unsafe` blocks and functions in test suites to ensure tests verify code through safe interfaces.
- **AST Pattern:** `syn::Expr::Unsafe` blocks or `unsafe fn` items inside test files, `#[cfg(test)]` modules, or test functions.
- **False-Positive Mitigation:** Can be suppressed with explicit `#[expect(opinionated::no_unsafe_in_tests, reason = "...")]` and an explanatory comment if FFI testing strictly requires it.
- **Severity:** `Severity::Error`.
- **Suggested Fix:** "Remove 'unsafe' block from test code; verify behavior strictly through safe public interfaces."

### Rule 14: `opinionated::centralized_command_execution`
- **Goal:** Prevent raw `std::process::Command::new(...)` invocations from scattering across business logic modules.
- **AST Pattern:** `Command::new(...)` invocations outside of dedicated command/tool modules (`src/tools/`, `src/commands/`).
- **False-Positive Mitigation:** Exempt tool/command modules and integration tests.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Encapsulate command execution and stdout parsing in a dedicated tool builder struct in a 'tools' module."

### Rule 15: `opinionated::clap_struct_encapsulation`
- **Goal:** Enforce encapsulation for Clap CLI models: struct fields must remain private, and command execution must be encapsulated in an associated `run(&self, ...)` method.
- **AST Pattern:** Structs deriving `clap::Args` or `clap::Parser` with public fields (unless `#[command(flatten)]`) or lacking an associated `run(&self, ...)` method.
- **False-Positive Mitigation:** Exempt flattened subcommands and root enum dispatchers.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Make Clap struct fields private and implement command execution in 'pub fn run(&self, ...)'."

### Rule 16: `opinionated::exit_code_hygiene`
- **Goal:** Prevent raw integer exit codes (`exit(1)`) and prevent `ExitCode` or `process::exit` calls from leaking into internal library modules.
- **AST Pattern:** Calls to `std::process::exit` in non-entrypoint files, raw integer literals passed to `exit(...)`, or library functions returning `std::process::ExitCode`.
- **False-Positive Mitigation:** Explicitly permitted in root `fn main() -> ExitCode` in entrypoint files.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Propagate errors via 'Result'; handle exit codes using 'ExitCode' strictly in 'main.rs'."

### Rule 17: `opinionated::idiomatic_option_bool_mapping`
- **Goal:** Flag `if let Some(...) = ... { ... } else { false }` (or `{ None }`) anti-patterns in favor of functional combinators.
- **AST Pattern:** `syn::ExprIf` matching `Some(...)` where the `else` branch evaluates to literal `false` or `None`.
- **False-Positive Mitigation:** Multi-statement `else` blocks with side-effects or early returns are exempted.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Replace with '.is_some_and(...)' or '.map(...).unwrap_or_default()'."

### Rule 18: `opinionated::no_wildcard_imports`
- **Goal:** Ban glob/wildcard imports (`use foo::*`), ensuring explicit dependency imports for readability and preventing symbol shadowing.
- **AST Pattern:** `syn::ItemUse` containing `syn::UseTree::Glob`.
- **False-Positive Mitigation:** Explicitly exempts prelude imports (`use googletest::prelude::*`, `use std::io::prelude::*`) and test modules/files.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Replace wildcard import with explicit symbol imports (e.g., 'use foo::{Bar, Baz};')."

### Rule 19: `opinionated::no_env_access_outside_config`
- **Goal:** Ban direct `std::env::var*` calls scattered throughout business logic, enforcing centralized configuration loading.
- **AST Pattern:** Invocations of `std::env::var`, `std::env::var_os`, `std::env::set_var`, or `std::env::remove_var` in production code.
- **False-Positive Mitigation:** Exempts configuration modules (`config.rs`, `settings.rs`), `build.rs`, and test files/functions.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Centralize environment variable reading in a dedicated configuration struct/module."

### Rule 20: `opinionated::single_match_to_let_else`
- **Goal:** Flag 2-arm match expressions unpacking a single variant (`Some`/`Ok`) while the other arm diverges, replacing nested indentation with `let ... else { ... };`.
- **AST Pattern:** `syn::ExprMatch` with 2 arms where one arm matches a single-variant pattern and the other arm's body diverges (`return`, `break`, `continue`, `panic!`, `bail!`).
- **False-Positive Mitigation:** Multi-arm matches or matches where neither arm diverges are unaffected.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Replace 'match' with idiomatic 'let Some(...) = expr else { ... };' to reduce indentation."

### Rule 21: `opinionated::raii_temp_directories`
- **Goal:** Forbid manual directory teardown (`fs::remove_dir_all(...)`) in test code, enforcing RAII temporary directory guards (`tempfile::TempDir`).
- **AST Pattern:** Calls to `remove_dir_all` or `std::fs::remove_dir_all` inside test functions or test modules.
- **False-Positive Mitigation:** Production code calling `remove_dir_all` as part of its domain logic is permitted.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Use an RAII temporary directory guard (e.g. 'tempfile::tempdir()') so cleanup is guaranteed on panic or test failure."

### Rule 22: `opinionated::no_println_in_libraries`
- **Goal:** Forbid `println!`, `print!`, `eprintln!`, `eprint!` in library crates, enforcing structured logging (`tracing`, `log`) or error propagation.
- **AST Pattern:** Invocations of printing macros in library modules.
- **False-Positive Mitigation:** Entry points (`main.rs`, `bin/*.rs`), `examples/`, and test functions/modules are exempted.
- **Severity:** `Severity::Warning`.
- **Suggested Fix:** "Use structured logging ('tracing::info!', 'log::info!') or return data to the caller instead of printing directly to standard output."

---

## 4. AST Visitor Framework, File Discovery & Configuration

### 4.1 `LintContext`
The `LintContext` struct provides rules with access to:
- `file_path: &Path`: Target file path.
- `source: &str`: Raw file source code.
- `lines: Vec<&str>`: Split source lines for comment inspection and line-number lookup.
- `is_main_or_lib: bool`: Indicates if the current file is `main.rs` or `lib.rs`.
- `is_test_file: bool`: Indicates if the file is in `tests/` or named `*_test.rs`.

### 4.2 `Rule` Trait
```rust
pub trait Rule: Send + Sync {
    /// Returns the unique rule identifier (e.g., "opinionated::no_inline_mods").
    fn name(&self) -> &'static str;

    /// Analyzes the parsed AST file and reports any diagnostics found.
    fn check_file(&self, ctx: &LintContext, file: &syn::File) -> Vec<Diagnostic>;
}
```

### 4.3 Clippy-Like Cargo Target & File Discovery
Rather than arbitrary filesystem directory walking, `code-review opinionated` mimics `cargo clippy` by parsing `Cargo.toml`:
- Locates the nearest `Cargo.toml` manifest in the hierarchy.
- For virtual workspaces, resolves `[workspace.members]` patterns (including wildcards like `crates/*`).
- Extracts standard Cargo targets: `src/` (lib and binaries), `tests/`, `examples/`, `benches/`, as well as explicit `[lib]` and `[[bin]]` path declarations.
- Prunes `target/`, `.git/`, `.jj/`, `.direnv/`, and hidden directories.
- Gracefully falls back to directory crawling if running outside of a Cargo workspace.

### 4.4 Rule Configuration & Disabling
Users can configure and disable rules at both project and code levels without conflicting with rustc or Clippy:

1. **Manifest Configuration (`Cargo.toml`):**
   Cargo's `[lints]` table is reserved strictly by Cargo for rustc-integrated tools (`rust`, `clippy`, `rustdoc`, `cargo`). Unrecognized tools under `[lints]` cause Cargo to forward unrecognized flags to `rustc`, triggering `error[E0602]`.
   
   Therefore, standard project-level configuration for external tools in Cargo uses `[package.metadata.opinionated.lints]` or `[workspace.metadata.opinionated.lints]`:
   ```toml
   [package.metadata.opinionated.lints]
   no_wildcard_imports = "allow"
   no_boxed_dyn_error = "deny"
   exit_code_hygiene = { level = "warn" }
   ```
   *(Note: `[lints.opinionated]` is also parsed if custom tool lint registration is enabled in a future Cargo version).*
   Supported levels: `"allow"` (suppressed), `"warn"`, `"deny"` (upgraded to error), `"forbid"` (upgraded to error).

2. **In-Code Suppression (Comments & Attributes):**
   - **Comment Directives (Recommended for Stable Rust):**
     Because `rustc` on stable Rust errors with `error[E0710]: unknown tool name` when encountering unrecognized attribute tools like `#[allow(opinionated::...)]`, comment directives are the conflict-free, 100% stable suppression mechanism:
     - Next-line suppression: `// opinionated:allow(rule_name)` (or `// opinionated:disable(rule_name)`) directly above the flagged statement.
     - Same-line suppression: `... // opinionated:allow(rule_name)` at the end of the line.
     - File-level suppression: `//! opinionated:allow(rule_name)` or `// opinionated:file-allow(rule_name)` at the top of the file.
     - Universal suppression: `// opinionated:allow` (omitting parentheses suppresses all opinionated rules for that line).
   - **Nightly Tool Attributes:**
     If `#![feature(register_tool)]` and `#![register_tool(opinionated)]` are enabled on nightly Rust, standard tool attributes `#[allow(opinionated::rule_name)]` and `#[expect(opinionated::rule_name)]` are also fully supported.

---

## 5. Verification Plan

1. **Unit Tests per Rule:**
   Each rule in `crates/opinionated/src/rules/` MUST have dedicated unit tests with positive snippets (triggering the lint) and negative snippets (valid idiomatic code passing without findings).
2. **Engine Tests:**
   Verifies directory traversal, Cargo target discovery, manifest lint level parsing, in-code suppression handling, and graceful syntax error handling.
3. **CLI Integration Tests:**
   Verifies command-line options (`--path`, `--format`, `--quiet`, `--fix`), formatted output in Console/Json/Markdown, and proper exit codes (0, 1, 2).
