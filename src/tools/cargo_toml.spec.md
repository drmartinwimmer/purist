# Module Specification: `cargo_toml` Lint Configurator

## 1. Module Purpose

The `cargo_toml` module (`src/tools/cargo_toml.rs`) provides robust, non-destructive programmatic modification of `Cargo.toml` files to inject, update, and remove strict Clippy lint configurations.

Manual configuration of extensive Clippy lint suites is error-prone, tedious, and often causes accidental stripping of custom comments or formatting when parsed and serialized by standard TOML serializers. This module uses `toml_edit` to ensure that existing comments, whitespace, indentation, and unrelated configuration tables remain strictly intact.

---

## 2. Public API / Contracts

### 2.1 Types and Data Structures

```rust
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Profile defining which preset of Clippy lints to inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LintProfile {
    /// Comprehensive safety, robustness, memory safety, numerics, and suppression hygiene rules (34 lints).
    #[default]
    Strict,
    /// Standard baseline safety and robustness rules (31 lints), omitting the strictest slicing
    /// and suppression bans (`string_slice`, `indexing_slicing`, and `allow_attributes`).
    Standard,
}

/// Result metadata returned after modifying or inspecting a Cargo.toml file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigureResult {
    /// True if the file contents were modified on disk; false if already up-to-date or no lints removed.
    pub modified: bool,
    /// For `configure_lints`: The number of lints configured in the targeted table.
    /// For `remove_lints`: The number of lints removed from the targeted table (or 0 if none found).
    pub lints_configured: usize,
}

/// Errors that can occur during Cargo.toml inspection or modification.
#[derive(Debug, Error)]
pub enum CargoTomlError {
    #[error("Manifest file not found at path: {path}")]
    ManifestNotFound { path: PathBuf },

    #[error("I/O error accessing manifest file: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Failed to parse TOML manifest: {source}")]
    Parse {
        #[from]
        source: toml_edit::TomlError,
    },

    /// Returned when an expected table (such as `[workspace]`, `[package]`, `[lints]`,
    /// or `[lints.clippy]`) exists in the document but is a scalar or array rather than
    /// a table or inline table.
    #[error("Invalid Cargo.toml structure: {reason}")]
    InvalidStructure { reason: String },
}
```

### 2.2 Public Functions

```rust
/// Configures strict Clippy lints in the specified Cargo.toml manifest.
///
/// Injects or updates the appropriate lints table with the rules defined in `profile`.
/// If the manifest is already fully compliant with the profile, no disk write occurs
/// and `ConfigureResult.modified` will be `false`.
pub fn configure_lints(
    manifest_path: &Path,
    profile: LintProfile,
) -> Result<ConfigureResult, CargoTomlError>;

/// Removes configured Clippy lints and associated tables from the specified Cargo.toml manifest.
///
/// Cleans up injected `[workspace.lints.clippy]` or `[lints.clippy]` tables while preserving
/// all other package metadata, dependencies, and unrelated tables and comments.
///
/// Returns `ConfigureResult` where:
/// - `lints_configured`: Count of removed lints (0 if none found).
/// - `modified`: `true` if file contents changed on disk, `false` otherwise.
///
/// If removing `clippy` leaves `[lints]` or `[workspace.lints]` empty (with no other keys
/// or subtables like `[lints.rust]`), the empty parent table is cleanly pruned.
pub fn remove_lints(
    manifest_path: &Path,
) -> Result<ConfigureResult, CargoTomlError>;
```

---

## 3. Invariants and Behavioral Guarantees

### Invariant 1: Preservation of Formatting and Comments
- All manipulations MUST be performed using `toml_edit::ImDocument` or `toml_edit::DocumentMut`.
- Existing comments (inline `# ...` comments, block comments, table header comments) MUST remain untouched in their exact positions.
- Existing whitespace, blank lines, and indentation styles MUST be preserved.
- Tables, keys, and values not targeted by the configuration (e.g. `[package]`, `[dependencies]`, `[features]`) MUST NOT be reordered, modified, or deleted.

### Invariant 2: Idempotency
- Running `configure_lints` on an already-configured `Cargo.toml` MUST produce byte-for-byte identical content.
- If no changes were required, `ConfigureResult.modified` MUST be `false`, and no write to disk should occur.
- Re-running the command MUST NOT duplicate lint keys or section headers.

### Invariant 3: Workspace vs Package Manifest Targeting (Virtual Manifest Safety)
Cargo rules dictate that `[lints]` tables are forbidden in virtual workspace manifests (manifests with `[workspace]` but without `[package]`). The configurator distinguishes three distinct cases:

1. **Virtual Workspace Manifest (`[workspace]` present, `[package]` absent):**
   - Configures ONLY `[workspace.lints.clippy]`.
   - MUST NOT inject `[lints]` or `[lints] workspace = true` (which would trigger Cargo parse errors).
2. **Root Package with Workspace (`[workspace]` present AND `[package]` present):**
   - Configures `[workspace.lints.clippy]`.
   - Injects or ensures `[lints] workspace = true` in the root package.
3. **Single Crate or Member Crate (`[workspace]` absent, `[package]` present):**
   - Configures `[lints.clippy]` directly.

Existing sub-tables under `[lints]` or `[workspace.lints]` (such as `[lints.rust]`) must be preserved without alteration.

### Invariant 4: Lint Profile Presets

#### `LintProfile::Strict` (34 Lints)
Injects all 34 rules set to `"warn"`:

1. **Don't Panic (11 lints):**
   - `string_slice = "warn"`
   - `indexing_slicing = "warn"`
   - `unwrap_used = "warn"`
   - `panic = "warn"`
   - `todo = "warn"`
   - `unimplemented = "warn"`
   - `unreachable = "warn"`
   - `get_unwrap = "warn"`
   - `unwrap_in_result = "warn"`
   - `unchecked_time_subtraction = "warn"`
   - `panic_in_result_fn = "warn"`

2. **Don't Fail Silently (5 lints):**
   - `let_underscore_future = "warn"`
   - `let_underscore_must_use = "warn"`
   - `unused_result_ok = "warn"`
   - `map_err_ignore = "warn"`
   - `assertions_on_result_states = "warn"`

3. **Don't Do Unsafe Things with Memory (5 lints):**
   - `mem_forget = "warn"`
   - `undocumented_unsafe_blocks = "warn"`
   - `multiple_unsafe_ops_per_block = "warn"`
   - `unnecessary_safety_doc = "warn"`
   - `unnecessary_safety_comment = "warn"`

4. **Don't Do Potentially Incorrect Things with Numbers (5 lints):**
   - `float_cmp = "warn"`
   - `float_cmp_const = "warn"`
   - `lossy_float_literal = "warn"`
   - `cast_sign_loss = "warn"`
   - `invalid_upcast_comparisons = "warn"`

5. **Don't Do Bad Things That Are Easy to Avoid (6 lints):**
   - `rc_mutex = "warn"`
   - `debug_assert_with_mut_call = "warn"`
   - `iter_not_returning_iterator = "warn"`
   - `expl_impl_clone_on_copy = "warn"`
   - `infallible_try_from = "warn"`
   - `dbg_macro = "warn"`

6. **Don't `allow` Your Way Around Lints (2 lints):**
   - `allow_attributes = "warn"`
   - `allow_attributes_without_reason = "warn"`

#### `LintProfile::Standard` (31 Lints)
Injects a baseline subset omitting `string_slice`, `indexing_slicing`, and `allow_attributes`, retaining the following 31 lints:
- **Don't Panic (9 lints):** `unwrap_used`, `panic`, `todo`, `unimplemented`, `unreachable`, `get_unwrap`, `unwrap_in_result`, `unchecked_time_subtraction`, `panic_in_result_fn`
- **Don't Fail Silently (5 lints):** `let_underscore_future`, `let_underscore_must_use`, `unused_result_ok`, `map_err_ignore`, `assertions_on_result_states`
- **Don't Do Unsafe Things with Memory (5 lints):** `mem_forget`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`, `unnecessary_safety_doc`, `unnecessary_safety_comment`
- **Don't Do Potentially Incorrect Things with Numbers (5 lints):** `float_cmp`, `float_cmp_const`, `lossy_float_literal`, `cast_sign_loss`, `invalid_upcast_comparisons`
- **Don't Do Bad Things That Are Easy to Avoid (6 lints):** `rc_mutex`, `debug_assert_with_mut_call`, `iter_not_returning_iterator`, `expl_impl_clone_on_copy`, `infallible_try_from`, `dbg_macro`
- **Don't `allow` Your Way Around Lints (1 lint):** `allow_attributes_without_reason`

### Invariant 5: Clean Removal and Table Pruning
- When `remove_lints` is executed, it strips the targeted `clippy` table.
- If removing `clippy` leaves the parent `[lints]` or `[workspace.lints]` table completely empty (no other keys, no other subtables like `[lints.rust]`), the empty parent table is pruned to maintain a clean manifest.
- If other sub-tables or keys remain in the parent table, the parent table is preserved.
- Returns `ConfigureResult { modified: true, lints_configured: N }` where `N` is the number of removed lints, or `ConfigureResult { modified: false, lints_configured: 0 }` if no clippy lints existed.

### Invariant 6: Structural Validation
- If any required or traversed key (e.g. `workspace`, `package`, `lints`, `clippy`) exists as a non-table scalar (e.g., `lints = "invalid"` or `workspace = 123`), the operation must fail immediately returning `CargoTomlError::InvalidStructure`.

---

## 4. Verification Plan

The module must include automated unit and integration tests verifying the following test cases:

1. **Comment and Whitespace Preservation Test:**
   - Input: A minimal `Cargo.toml` containing leading, inline, and trailing comments, irregular blank lines, and diverse dependencies.
   - Action: Run `configure_lints`.
   - Verification: All original comments and formatting remain intact; lints table is cleanly added.

2. **Idempotency Test:**
   - Input: The output from the first `configure_lints` run.
   - Action: Run `configure_lints` a second time.
   - Verification: Returns `ConfigureResult { modified: false, lints_configured: 34 }` and file content is byte-for-byte identical.

3. **Workspace vs Package Manifest Tests:**
   - **Test 3a (Virtual Workspace Manifest):**
     - Input: `[workspace] members = ["crates/*"]` without `[package]`.
     - Verification: Injects `[workspace.lints.clippy]`. Verifies that `[lints]` and `[lints] workspace = true` are NOT added.
   - **Test 3b (Root Package with Workspace):**
     - Input: `[workspace]` and `[package]` both present.
     - Verification: Injects `[workspace.lints.clippy]` AND ensures `[lints] workspace = true`.
   - **Test 3c (Single Crate Manifest):**
     - Input: `[package]` only (no `[workspace]`).
     - Verification: Injects `[lints.clippy]` directly.

4. **Profile Differences Test:**
   - Verify that `LintProfile::Strict` injects exactly 34 lints.
   - Verify that `LintProfile::Standard` injects exactly 31 lints (omitting `string_slice`, `indexing_slicing`, and `allow_attributes`).

5. **Removal and Pruning Test:**
   - Input: A `Cargo.toml` with configured lints and no other `[lints]` entries.
   - Action: Run `remove_lints`.
   - Verification: Clippy table is removed, empty parent `[lints]` table is cleanly pruned, returns `ConfigureResult { modified: true, lints_configured: 34 }`.
   - Running `remove_lints` again returns `ConfigureResult { modified: false, lints_configured: 0 }`.

6. **Error Conditions Test:**
   - Path does not exist -> returns `CargoTomlError::ManifestNotFound`.
   - File contains malformed TOML -> returns `CargoTomlError::Parse`.
   - `lints` or `workspace` is a scalar value -> returns `CargoTomlError::InvalidStructure`.
