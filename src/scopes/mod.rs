//! # AST Scopes
//!
//! Provides stack-based lexical scopes and scoped closures for AST traversal across Purist rules:
//! - [`TestScope`]: Tracks whether traversal is currently within test files, `#[cfg(test)]` modules, or test functions.
//! - [`ClapScope`]: Tracks enclosing Clap CLI command models, argument fields, and command metadata.
//! - [`TypeScope`]: Tracks enclosing structs, enums, variants, and implementation blocks.
//! - [`DepthScope`]: Measures control flow nesting depth using a stack for function/closure boundaries.
//! - [`FlagScope`]: Tracks arbitrary boolean scopes using a stack.
//!
//! Also exports derive macros for automatic trait implementations:
//! - [`WithTestScope`]: Automatically implements [`WithTestScope`].
//! - [`WithClapScope`]: Automatically implements [`WithClapScope`].
//! - [`WithTypeScope`]: Automatically implements [`WithTypeScope`].
//! - [`WithDepthScope`]: Automatically implements [`WithDepthScope`].

pub mod block_scope;
pub mod clap_scope;
pub mod depth_scope;
pub mod guard;
pub mod main_scope;
pub mod suppression_scope;
pub mod test_scope;
pub mod type_scope;

pub use block_scope::{BlockScope, WithBlockScope};
pub use clap_scope::{
    ClapScope, ClapStructInfo, WithClapScope, derives_clap, is_cli_or_command_struct_name,
    is_command_execution_fn_name, is_command_struct_name, is_flattened_field,
};
pub use depth_scope::{DepthScope, FlagScope, WithDepthScope};
pub use guard::{ScopeGuard, run_with_block, run_with_exact_flag, run_with_flag, run_with_scope};
pub use main_scope::{MainScope, WithMainScope};
pub use purist_derive::{
    WithBlockScope, WithClapScope, WithDepthScope, WithMainScope, WithSuppressionScope,
    WithTestScope, WithTypeScope,
};
pub use suppression_scope::{SuppressionScope, WithSuppressionScope};
pub use test_scope::{TestScope, TestScopeState, WithTestScope};
pub use type_scope::{ContainerKind, TypeScope, WithTypeScope};

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;

    #[derive(WithTestScope)]
    struct DerivedTestVisitor {
        test_scope: TestScope,
    }

    #[derive(WithClapScope)]
    struct DerivedClapVisitor {
        clap_scope: ClapScope,
    }

    #[derive(WithTypeScope)]
    struct DerivedTypeVisitor {
        type_scope: TypeScope,
    }

    #[derive(WithDepthScope)]
    struct DerivedDepthVisitor {
        depth_scope: DepthScope,
    }

    #[derive(WithBlockScope)]
    struct DerivedBlockVisitor {
        block_scope: BlockScope<String, i32>,
    }

    #[derive(WithMainScope)]
    struct DerivedMainVisitor {
        main_scope: MainScope,
    }

    #[derive(WithSuppressionScope)]
    struct DerivedSuppressionVisitor {
        suppression_scope: SuppressionScope,
    }

    #[derive(WithTestScope, WithClapScope, WithTypeScope, WithDepthScope)]
    struct MultiScopeVisitor {
        test_scope: TestScope,
        clap_scope: ClapScope,
        type_scope: TypeScope,
        depth_scope: DepthScope,
    }

    #[derive(WithTestScope, WithClapScope, WithTypeScope, WithDepthScope)]
    struct MultiScopeBareAttributeVisitor {
        #[scope]
        custom_test: TestScope,
        #[scope]
        custom_clap: ClapScope,
        #[scope]
        custom_type: TypeScope,
        #[scope]
        custom_depth: DepthScope,
    }

    #[derive(WithTestScope, WithClapScope, WithTypeScope, WithDepthScope)]
    struct MultiScopeSpecificAttributeVisitor {
        #[test_scope]
        t: TestScope,
        #[clap_scope]
        c: ClapScope,
        #[type_scope]
        ty: TypeScope,
        #[depth_scope]
        d: DepthScope,
    }

    #[derive(WithTestScope, WithClapScope, WithTypeScope, WithDepthScope)]
    struct MultiScopeParameterizedAttributeVisitor {
        #[scope(test)]
        t: TestScope,
        #[scope(clap)]
        c: ClapScope,
        #[scope(type)]
        ty: TypeScope,
        #[scope(depth)]
        d: DepthScope,
    }

    #[googletest::test]
    fn derive_macros_generate_working_scope_traits() -> Result<(), Box<dyn std::error::Error>> {
        let mut test_v = DerivedTestVisitor {
            test_scope: TestScope::new(false),
        };
        let test_mod: syn::ItemMod = syn::parse_str("#[cfg(test)] mod tests {}")?;
        test_v.with_test_mod(&test_mod.attrs, |v| {
            assert_that!(v.test_scope_mut().is_in_test(), eq(true));
        });
        assert_that!(test_v.test_scope_mut().is_in_test(), eq(false));

        let mut clap_v = DerivedClapVisitor {
            clap_scope: ClapScope::new(),
        };
        let clap_struct: syn::ItemStruct =
            syn::parse_str("#[derive(clap::Args)] struct CmdArgs { verbose: bool }")?;
        clap_v.with_clap_struct(&clap_struct, |v| {
            assert_that!(v.clap_scope_mut().is_in_clap_struct(), eq(true));
        });
        assert_that!(clap_v.clap_scope_mut().is_in_clap_struct(), eq(false));

        let mut type_v = DerivedTypeVisitor {
            type_scope: TypeScope::new(),
        };
        let item_struct: syn::ItemStruct = syn::parse_str("struct User;")?;
        type_v.with_type_struct(&item_struct.ident, |v| {
            assert_that!(
                v.type_scope_mut().container_description(),
                some(eq("struct 'User'"))
            );
        });
        assert_that!(type_v.type_scope_mut().container_description(), none());

        let mut depth_v = DerivedDepthVisitor {
            depth_scope: DepthScope::new(),
        };
        depth_v.with_depth_step(|v| {
            assert_that!(v.depth_scope_mut().get(), eq(1));
        });
        assert_that!(depth_v.depth_scope_mut().get(), eq(0));

        let mut block_v = DerivedBlockVisitor {
            block_scope: BlockScope::new(),
        };
        let mut bindings = std::collections::HashMap::new();
        bindings.insert("val".to_string(), 42);
        block_v.with_block(bindings, |v| {
            assert_that!(v.block_scope_mut().get("val"), some(eq(&42)));
        });
        assert_that!(block_v.block_scope_mut().get("val"), none());

        let mut main_v = DerivedMainVisitor {
            main_scope: MainScope::new(),
        };
        main_v.with_main_fn(true, |v| {
            assert_that!(v.main_scope_mut().is_in_main(), eq(true));
        });
        assert_that!(main_v.main_scope_mut().is_in_main(), eq(false));

        let mut supp_v = DerivedSuppressionVisitor {
            suppression_scope: SuppressionScope::new(),
        };
        supp_v.with_suppression(true, |v| {
            assert_that!(v.suppression_scope_mut().is_suppressed(), eq(true));
        });
        assert_that!(supp_v.suppression_scope_mut().is_suppressed(), eq(false));

        let mut multi = MultiScopeVisitor {
            test_scope: TestScope::new(false),
            clap_scope: ClapScope::new(),
            type_scope: TypeScope::new(),
            depth_scope: DepthScope::new(),
        };
        multi.with_test_mod(&test_mod.attrs, |v| {
            v.with_depth_step(|v2| {
                assert_that!(v2.test_scope_mut().is_in_test(), eq(true));
                assert_that!(v2.depth_scope_mut().get(), eq(1));
            });
        });
        assert_that!(multi.test_scope_mut().is_in_test(), eq(false));
        assert_that!(multi.depth_scope_mut().get(), eq(0));

        Ok(())
    }

    #[googletest::test]
    fn multi_scope_attributes_derive_without_clashing() -> Result<(), Box<dyn std::error::Error>> {
        let test_mod: syn::ItemMod = syn::parse_str("#[cfg(test)] mod tests {}")?;

        let mut bare_multi = MultiScopeBareAttributeVisitor {
            custom_test: TestScope::new(false),
            custom_clap: ClapScope::new(),
            custom_type: TypeScope::new(),
            custom_depth: DepthScope::new(),
        };
        bare_multi.with_test_mod(&test_mod.attrs, |v| {
            v.with_depth_step(|v2| {
                assert_that!(v2.test_scope_mut().is_in_test(), eq(true));
                assert_that!(v2.depth_scope_mut().get(), eq(1));
            });
        });
        assert_that!(bare_multi.test_scope_mut().is_in_test(), eq(false));
        assert_that!(bare_multi.depth_scope_mut().get(), eq(0));

        let mut specific_multi = MultiScopeSpecificAttributeVisitor {
            t: TestScope::new(false),
            c: ClapScope::new(),
            ty: TypeScope::new(),
            d: DepthScope::new(),
        };
        specific_multi.with_test_mod(&test_mod.attrs, |v| {
            v.with_depth_step(|v2| {
                assert_that!(v2.test_scope_mut().is_in_test(), eq(true));
                assert_that!(v2.depth_scope_mut().get(), eq(1));
            });
        });
        assert_that!(specific_multi.test_scope_mut().is_in_test(), eq(false));
        assert_that!(specific_multi.depth_scope_mut().get(), eq(0));

        let mut param_multi = MultiScopeParameterizedAttributeVisitor {
            t: TestScope::new(false),
            c: ClapScope::new(),
            ty: TypeScope::new(),
            d: DepthScope::new(),
        };
        param_multi.with_test_mod(&test_mod.attrs, |v| {
            v.with_depth_step(|v2| {
                assert_that!(v2.test_scope_mut().is_in_test(), eq(true));
                assert_that!(v2.depth_scope_mut().get(), eq(1));
            });
        });
        assert_that!(param_multi.test_scope_mut().is_in_test(), eq(false));
        assert_that!(param_multi.depth_scope_mut().get(), eq(0));

        Ok(())
    }

    #[googletest::test]
    fn scope_stack_tracks_scopes_correctly() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = TestScope::new(false);
        assert_that!(tracker.is_in_test(), eq(false));

        let test_mod: syn::ItemMod = syn::parse_str("#[cfg(test)] mod tests {}")?;
        tracker.push_mod(&test_mod.attrs);
        assert_that!(tracker.is_in_test(), eq(true));
        assert_that!(tracker.is_in_test_module(), eq(true));
        tracker.pop();
        assert_that!(tracker.is_in_test(), eq(false));

        let test_fn: syn::ItemFn = syn::parse_str("#[test] fn check() {}")?;
        tracker.push_fn(&test_fn.attrs);
        assert_that!(tracker.is_in_test(), eq(true));
        assert_that!(tracker.is_in_test_fn(), eq(true));
        tracker.pop();
        assert_that!(tracker.is_in_test(), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn clap_scope_identifies_clap_structs() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = ClapScope::new();
        assert_that!(tracker.is_in_clap_struct(), eq(false));

        let non_clap: syn::ItemStruct = syn::parse_str("struct ServerConfig { timeout: u64 }")?;
        tracker.push_struct(&non_clap);
        assert_that!(tracker.is_in_clap_struct(), eq(false));
        tracker.pop();

        let clap_struct: syn::ItemStruct =
            syn::parse_str("#[derive(clap::Args)] struct BuildCommand { release: bool }")?;
        tracker.push_struct(&clap_struct);
        assert_that!(tracker.is_in_clap_struct(), eq(true));
        assert_that!(tracker.current_struct_name(), some(eq("BuildCommand")));
        assert_that!(tracker.is_current_command(), eq(true));
        tracker.pop();
        assert_that!(tracker.is_in_clap_struct(), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn type_scope_formats_containers() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = TypeScope::new();
        assert_that!(tracker.container_description(), none());

        let item_struct: syn::ItemStruct = syn::parse_str("struct User;")?;
        tracker.push_struct(&item_struct.ident);
        assert_that!(tracker.container_description(), some(eq("struct 'User'")));
        tracker.pop();
        assert_that!(tracker.container_description(), none());

        let item_enum: syn::ItemEnum = syn::parse_str("enum Action { Run }")?;
        tracker.push_enum(&item_enum.ident);
        let variant = item_enum.variants.first().ok_or("expected variant")?;
        tracker.push_variant(&variant.ident);
        assert_that!(
            tracker.container_description(),
            some(eq("enum variant 'Action::Run'"))
        );
        tracker.pop(); // pop variant
        assert_that!(tracker.container_description(), some(eq("enum 'Action'")));
        tracker.pop(); // pop enum
        assert_that!(tracker.container_description(), none());
        Ok(())
    }

    #[googletest::test]
    fn depth_scope_tracks_nesting_levels() {
        let mut tracker = DepthScope::new();
        assert_that!(tracker.get(), eq(0));

        tracker.enter();
        assert_that!(tracker.get(), eq(1));

        tracker.enter();
        assert_that!(tracker.get(), eq(2));

        tracker.push_root();
        assert_that!(tracker.get(), eq(0));

        tracker.enter();
        assert_that!(tracker.get(), eq(1));
        tracker.exit();

        tracker.pop_root();
        assert_that!(tracker.get(), eq(2));

        tracker.exit();
        assert_that!(tracker.get(), eq(1));
        tracker.exit();
        assert_that!(tracker.get(), eq(0));
    }

    #[googletest::test]
    fn scoped_closures_restore_state_on_normal_exit_and_nesting()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = TestScope::new(false);
        let test_mod: syn::ItemMod = syn::parse_str("#[cfg(test)] mod tests {}")?;
        let test_fn: syn::ItemFn = syn::parse_str("#[test] fn my_test() {}")?;

        tracker.with_test_mod(&test_mod.attrs, |t| {
            assert_that!(t.is_in_test(), eq(true));
            assert_that!(t.is_in_test_module(), eq(true));
            assert_that!(t.is_in_test_fn(), eq(false));

            t.with_test_fn(&test_fn.attrs, |t2| {
                assert_that!(t2.is_in_test(), eq(true));
                assert_that!(t2.is_in_test_fn(), eq(true));
            });

            assert_that!(t.is_in_test_fn(), eq(false));
            assert_that!(t.is_in_test_module(), eq(true));
        });

        assert_that!(tracker.is_in_test(), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn clap_scope_closure_restores_state() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = ClapScope::new();
        let clap_struct: syn::ItemStruct =
            syn::parse_str("#[derive(clap::Args)] struct BuildCommand { release: bool }")?;

        tracker.with_clap_struct(&clap_struct, |t| {
            assert_that!(t.is_in_clap_struct(), eq(true));
            assert_that!(t.current_struct_name(), some(eq("BuildCommand")));
        });

        assert_that!(tracker.is_in_clap_struct(), eq(false));
        Ok(())
    }

    #[googletest::test]
    fn type_scope_closure_restores_state() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = TypeScope::new();
        let item_struct: syn::ItemStruct = syn::parse_str("struct User;")?;

        tracker.with_type_struct(&item_struct.ident, |t| {
            assert_that!(t.container_description(), some(eq("struct 'User'")));
        });

        assert_that!(tracker.container_description(), none());
        Ok(())
    }

    #[googletest::test]
    fn depth_and_flag_scope_closures_restore_state() {
        let mut depth = DepthScope::new();
        depth.with_depth_step(|d| {
            assert_that!(d.get(), eq(1));
            d.with_depth_root(|d2| {
                assert_that!(d2.get(), eq(0));
            });
            assert_that!(d.get(), eq(1));
        });
        assert_that!(depth.get(), eq(0));

        let mut flag = FlagScope::new();
        flag.with_flag(true, |f| {
            assert_that!(f.is_active(), eq(true));
        });
        assert_that!(flag.is_active(), eq(false));
    }
}
