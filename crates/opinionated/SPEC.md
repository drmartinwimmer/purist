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

---

## 4. AST Visitor Framework & Engine

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

### 4.3 `OpinionatedEngine`
The engine aggregates all registered rules and executes them against target files or directories:
- Accepts a single file or a directory path.
- Collects all `.rs` files while respecting directory pruning invariants.
- Uses `syn::parse_file` to construct the AST.
- On parse failure, appends an error-level `Diagnostic` with the syn parse error span.
- Aggregates all emitted diagnostics into a `DiagnosticReport`.

---

## 5. Verification Plan

1. **Unit Tests per Rule:**
   Each rule in `crates/opinionated/src/rules/` MUST have dedicated unit tests with positive snippets (triggering the lint) and negative snippets (valid idiomatic code passing without findings).
2. **Engine Tests:**
   Verifies directory traversal, skipping of `target/` and `.jj/`, and graceful handling of invalid syntax.
3. **CLI Integration Tests:**
   Verifies command-line options (`--path`, `--format`, `--quiet`, `--fix`), formatted output in Console/Json/Markdown, and proper exit codes (0, 1, 2).
