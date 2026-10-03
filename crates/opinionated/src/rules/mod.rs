pub mod clippy_suppress;
pub mod error_types;
pub mod free_functions;
pub mod no_inline_mods;
pub mod no_redundant_conversions;
pub mod path_resolution;
pub mod test_patterns;

pub use clippy_suppress::ClippySuppressRule;
pub use error_types::ErrorTypesRule;
pub use free_functions::FreeFunctionsRule;
pub use no_inline_mods::NoInlineModsRule;
pub use no_redundant_conversions::NoRedundantConversionsRule;
pub use path_resolution::PathResolutionRule;
pub use test_patterns::TestPatternsRule;

use crate::engine::Rule;

/// Returns a collection of all standard opinionated static analysis rules.
pub fn default_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(NoInlineModsRule),
        Box::new(FreeFunctionsRule),
        Box::new(PathResolutionRule),
        Box::new(ErrorTypesRule),
        Box::new(ClippySuppressRule),
        Box::new(TestPatternsRule),
        Box::new(NoRedundantConversionsRule),
    ]
}
