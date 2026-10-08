//! # AST Scope Trackers
//!
//! Provides RAII-based lexical scope trackers for AST traversal across Purist rules:
//! - [`TestScopeTracker`]: Tracks whether traversal is currently within test files, `#[cfg(test)]` modules, or test functions.
//! - [`ClapScopeTracker`]: Tracks enclosing Clap CLI command models, argument fields, and command metadata.
//! - [`TypeScopeTracker`]: Tracks enclosing structs, enums, variants, and implementation blocks.
//! - [`DepthTracker`]: Measures control flow nesting depth with automatic RAII unwind.
//! - [`FlagScopeTracker`]: Tracks arbitrary boolean scopes using RAII guards.

pub mod clap_scope;
pub mod depth_scope;
pub mod guard;
pub mod test_scope;
pub mod type_scope;

pub use clap_scope::{
    ClapScopeState, ClapScopeTracker, ClapStructInfo, derives_clap, is_cli_or_command_struct_name,
    is_command_execution_fn_name, is_command_struct_name, is_flattened_field,
};
pub use depth_scope::{DepthTracker, FlagScopeTracker};
pub use guard::{RefScopeGuard, ScopeGuard};
pub use test_scope::{TestScopeState, TestScopeTracker};
pub use type_scope::{ContainerKind, TypeScopeState, TypeScopeTracker};

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[googletest::test]
    fn scope_tracker_raii_unwinds_correctly() -> Result<(), Box<dyn std::error::Error>> {
        let tracker = TestScopeTracker::new(false);
        assert_that!(tracker.is_in_test(), eq(false));

        let test_mod: syn::ItemMod = syn::parse_str("#[cfg(test)] mod tests {}")?;
        {
            let _guard = tracker.enter_mod(&test_mod.attrs);
            assert_that!(tracker.is_in_test(), eq(true));
            assert_that!(tracker.is_in_test_module(), eq(true));
        }
        assert_that!(tracker.is_in_test(), eq(false));

        let test_fn: syn::ItemFn = syn::parse_str("#[test] fn check() {}")?;
        {
            let _guard = tracker.enter_fn(&test_fn.attrs);
            assert_that!(tracker.is_in_test(), eq(true));
            assert_that!(tracker.is_in_test_fn(), eq(true));
        }
        assert_that!(tracker.is_in_test(), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn clap_scope_tracker_identifies_clap_structs() -> Result<(), Box<dyn std::error::Error>> {
        let tracker = ClapScopeTracker::new();
        assert_that!(tracker.is_in_clap_struct(), eq(false));

        let non_clap: syn::ItemStruct = syn::parse_str("struct ServerConfig { timeout: u64 }")?;
        {
            let _guard = tracker.enter_struct(&non_clap);
            assert_that!(tracker.is_in_clap_struct(), eq(false));
        }

        let clap_struct: syn::ItemStruct =
            syn::parse_str("#[derive(clap::Args)] struct BuildCommand { release: bool }")?;
        {
            let _guard = tracker.enter_struct(&clap_struct);
            assert_that!(tracker.is_in_clap_struct(), eq(true));
            assert_that!(tracker.current_struct_name(), some(eq("BuildCommand")));
            assert_that!(tracker.is_current_command(), eq(true));
        }
        assert_that!(tracker.is_in_clap_struct(), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn type_scope_tracker_formats_containers() -> Result<(), Box<dyn std::error::Error>> {
        let tracker = TypeScopeTracker::new();
        assert_that!(tracker.container_description(), none());

        let item_struct: syn::ItemStruct = syn::parse_str("struct User;")?;
        {
            let _guard = tracker.enter_struct(&item_struct.ident);
            assert_that!(tracker.container_description(), some(eq("struct 'User'")));
        }
        assert_that!(tracker.container_description(), none());

        let item_enum: syn::ItemEnum = syn::parse_str("enum Action { Run }")?;
        {
            let _enum_guard = tracker.enter_enum(&item_enum.ident);
            let variant = item_enum.variants.first().ok_or("expected variant")?;
            let _variant_guard = tracker.enter_variant(&variant.ident);
            assert_that!(
                tracker.container_description(),
                some(eq("enum variant 'Action::Run'"))
            );
        }
        assert_that!(tracker.container_description(), none());
        Ok(())
    }

    #[googletest::test]
    fn depth_tracker_tracks_nesting_levels() -> Result<(), Box<dyn std::error::Error>> {
        let tracker = DepthTracker::new();
        assert_that!(tracker.get(), eq(0));

        {
            let _g1 = tracker.enter();
            assert_that!(tracker.get(), eq(1));
            {
                let _g2 = tracker.enter();
                assert_that!(tracker.get(), eq(2));
                {
                    let _g_reset = tracker.reset();
                    assert_that!(tracker.get(), eq(0));
                }
                assert_that!(tracker.get(), eq(2));
            }
            assert_that!(tracker.get(), eq(1));
        }
        assert_that!(tracker.get(), eq(0));
        Ok(())
    }
}
