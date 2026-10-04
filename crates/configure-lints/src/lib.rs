pub mod cargo_toml;

pub use cargo_toml::{CargoTomlError, ConfigureResult, LintProfile, configure_lints, remove_lints};
use std::path::{Path, PathBuf};

/// Command-line arguments for configuring or removing Clippy lints in Cargo.toml.
#[derive(clap::Args, Debug, Clone, PartialEq, Eq)]
pub struct ConfigureLintsCommand {
    /// Path to the Cargo.toml manifest to configure
    #[arg(long, default_value = "Cargo.toml")]
    manifest_path: PathBuf,

    /// Lint profile preset to inject (strict or standard)
    #[arg(long, value_enum, default_value_t = LintProfile::Strict)]
    profile: LintProfile,

    /// Remove configured lints instead of injecting them
    #[arg(long)]
    remove: bool,

    /// Silence non-essential logging output
    #[arg(short, long)]
    quiet: bool,
}

impl ConfigureLintsCommand {
    /// Creates a new `ConfigureLintsCommand` instance.
    pub fn new(
        manifest_path: impl Into<PathBuf>,
        profile: LintProfile,
        remove: bool,
        quiet: bool,
    ) -> Self {
        Self {
            manifest_path: manifest_path.into(),
            profile,
            remove,
            quiet,
        }
    }

    /// Returns the manifest path targeted by this command.
    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    /// Returns the lint profile preset.
    pub fn profile(&self) -> LintProfile {
        self.profile
    }

    /// Returns whether lints should be removed rather than configured.
    pub fn is_remove(&self) -> bool {
        self.remove
    }

    /// Returns whether logging output is suppressed.
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    /// Executes the configuration or removal of Clippy lints in the target manifest.
    pub fn run(self) -> Result<(), CargoTomlError> {
        if self.remove {
            let result = remove_lints(&self.manifest_path)?;
            if !self.quiet {
                if result.modified {
                    eprintln!(
                        "Removed {} Clippy lints from '{}'.",
                        result.lints_configured,
                        self.manifest_path.display()
                    );
                } else {
                    eprintln!(
                        "No Clippy lints found in '{}'. Manifest unchanged.",
                        self.manifest_path.display()
                    );
                }
            }
        } else {
            let result = configure_lints(&self.manifest_path, self.profile)?;
            if !self.quiet {
                if result.modified {
                    eprintln!(
                        "Configured {} Clippy lints ({:?} profile) in '{}'.",
                        result.lints_configured,
                        self.profile,
                        self.manifest_path.display()
                    );
                } else {
                    eprintln!(
                        "Manifest '{}' already configured with {} Clippy lints ({:?} profile). Manifest unchanged.",
                        self.manifest_path.display(),
                        result.lints_configured,
                        self.profile
                    );
                }
            }
        }
        Ok(())
    }
}
