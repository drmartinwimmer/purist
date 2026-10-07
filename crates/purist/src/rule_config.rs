//! Configuration options for specific purist rules parsed from `Cargo.toml`.

/// Configuration options specific to `max_file_lines`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaxFileLinesConfig {
    pub max_production_lines: Option<usize>,
    pub max_total_lines: Option<usize>,
}

/// Configuration options specific to `max_nesting_depth`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaxNestingDepthConfig {
    pub max_depth: Option<usize>,
}

impl MaxFileLinesConfig {
    /// Parses configuration from a `[package.metadata.purist]` or sub-table.
    pub fn parse_from_metadata(&mut self, table: &toml_edit::Table, warnings: &mut Vec<String>) {
        if let Some(item) = table.get("max_production_lines")
            && let Some(u) = parse_usize_item("max_production_lines", item, warnings)
        {
            self.max_production_lines = Some(u);
        }
        if let Some(item) = table.get("max_total_lines")
            && let Some(u) = parse_usize_item("max_total_lines", item, warnings)
        {
            self.max_total_lines = Some(u);
        }
        if let Some(sub) = table.get("max_file_lines") {
            if let Some(sub_table) = sub.as_table() {
                self.parse_from_rule_table(sub_table, warnings);
            } else {
                warnings.push(
                    "Invalid type for 'max_file_lines' in Cargo.toml: expected a table."
                        .to_string(),
                );
            }
        }
    }

    /// Parses options from a rule configuration table.
    pub fn parse_from_rule_table(&mut self, table: &toml_edit::Table, warnings: &mut Vec<String>) {
        if let Some(item) = table.get("max_production_lines")
            && let Some(u) = parse_usize_item("max_production_lines", item, warnings)
        {
            self.max_production_lines = Some(u);
        }
        if let Some(item) = table.get("max_total_lines")
            && let Some(u) = parse_usize_item("max_total_lines", item, warnings)
        {
            self.max_total_lines = Some(u);
        }
    }

    /// Parses options from an inline table.
    pub fn parse_from_inline_table(
        &mut self,
        table: &toml_edit::InlineTable,
        warnings: &mut Vec<String>,
    ) {
        if let Some(item) = table.get("max_production_lines") {
            if let Some(val) = item.as_integer() {
                match usize::try_from(val) {
                    Ok(u) => self.max_production_lines = Some(u),
                    Err(_) => warnings.push(format!(
                        "Invalid value '{val}' for 'max_production_lines' in Cargo.toml: expected a non-negative integer."
                    )),
                }
            } else {
                warnings.push(
                    "Invalid type for 'max_production_lines' in Cargo.toml: expected an integer."
                        .to_string(),
                );
            }
        }
        if let Some(item) = table.get("max_total_lines") {
            if let Some(val) = item.as_integer() {
                match usize::try_from(val) {
                    Ok(u) => self.max_total_lines = Some(u),
                    Err(_) => warnings.push(format!(
                        "Invalid value '{val}' for 'max_total_lines' in Cargo.toml: expected a non-negative integer."
                    )),
                }
            } else {
                warnings.push(
                    "Invalid type for 'max_total_lines' in Cargo.toml: expected an integer."
                        .to_string(),
                );
            }
        }
    }
}

/// Configuration options specific to `lib_facade_hygiene`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibFacadeHygieneConfig {
    pub max_production_lines: Option<usize>,
    pub max_fn_lines: Option<usize>,
}

impl LibFacadeHygieneConfig {
    /// Parses configuration from a `[package.metadata.purist]` or sub-table.
    pub fn parse_from_metadata(&mut self, table: &toml_edit::Table, warnings: &mut Vec<String>) {
        if let Some(item) = table.get("max_lib_production_lines")
            && let Some(u) = parse_usize_item("max_lib_production_lines", item, warnings)
        {
            self.max_production_lines = Some(u);
        }
        if let Some(item) = table.get("max_lib_fn_lines")
            && let Some(u) = parse_usize_item("max_lib_fn_lines", item, warnings)
        {
            self.max_fn_lines = Some(u);
        }
        if let Some(sub) = table.get("lib_facade_hygiene") {
            if let Some(sub_table) = sub.as_table() {
                self.parse_from_rule_table(sub_table, warnings);
            } else {
                warnings.push(
                    "Invalid type for 'lib_facade_hygiene' in Cargo.toml: expected a table."
                        .to_string(),
                );
            }
        }
    }

    /// Parses options from a rule configuration table.
    pub fn parse_from_rule_table(&mut self, table: &toml_edit::Table, warnings: &mut Vec<String>) {
        if let Some(item) = table.get("max_production_lines")
            && let Some(u) = parse_usize_item("max_production_lines", item, warnings)
        {
            self.max_production_lines = Some(u);
        }
        if let Some(item) = table.get("max_fn_lines")
            && let Some(u) = parse_usize_item("max_fn_lines", item, warnings)
        {
            self.max_fn_lines = Some(u);
        }
    }

    /// Parses options from an inline table.
    pub fn parse_from_inline_table(
        &mut self,
        table: &toml_edit::InlineTable,
        warnings: &mut Vec<String>,
    ) {
        if let Some(item) = table.get("max_production_lines") {
            if let Some(val) = item.as_integer() {
                match usize::try_from(val) {
                    Ok(u) => self.max_production_lines = Some(u),
                    Err(_) => warnings.push(format!(
                        "Invalid value '{val}' for 'max_production_lines' in Cargo.toml: expected a non-negative integer."
                    )),
                }
            } else {
                warnings.push(
                    "Invalid type for 'max_production_lines' in Cargo.toml: expected an integer."
                        .to_string(),
                );
            }
        }
        if let Some(item) = table.get("max_fn_lines") {
            if let Some(val) = item.as_integer() {
                match usize::try_from(val) {
                    Ok(u) => self.max_fn_lines = Some(u),
                    Err(_) => warnings.push(format!(
                        "Invalid value '{val}' for 'max_fn_lines' in Cargo.toml: expected a non-negative integer."
                    )),
                }
            } else {
                warnings.push(
                    "Invalid type for 'max_fn_lines' in Cargo.toml: expected an integer."
                        .to_string(),
                );
            }
        }
    }
}

pub fn parse_usize_item(
    key: &str,
    item: &toml_edit::Item,
    warnings: &mut Vec<String>,
) -> Option<usize> {
    if let Some(val) = item.as_integer() {
        match usize::try_from(val) {
            Ok(u) => Some(u),
            Err(_) => {
                warnings.push(format!(
                    "Invalid value '{val}' for '{key}' in Cargo.toml: expected a non-negative integer."
                ));
                None
            }
        }
    } else {
        warnings.push(format!(
            "Invalid type for '{key}' in Cargo.toml: expected an integer."
        ));
        None
    }
}
