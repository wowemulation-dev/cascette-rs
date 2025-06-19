//! Output formatting utilities for the CLI
//!
//! This module provides utilities for formatting output in various styles
//! including tables, colored text, and structured displays.

use comfy_table::{Attribute, Cell, CellAlignment, Color, ContentArrangement, Table, presets};
use owo_colors::OwoColorize;

/// Style configuration for output formatting
pub struct OutputStyle {
    /// Whether to use colors in output
    pub use_color: bool,
    /// Whether to use Unicode characters for borders
    pub use_unicode: bool,
}

impl Default for OutputStyle {
    fn default() -> Self {
        Self {
            // Check if NO_COLOR env var is set
            use_color: std::env::var("NO_COLOR").is_err(),
            use_unicode: true,
        }
    }
}

impl OutputStyle {
    /// Create a new output style
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable colors in output
    #[must_use]
    pub fn no_color(mut self) -> Self {
        self.use_color = false;
        self
    }

    /// Use ASCII characters instead of Unicode
    #[must_use]
    pub fn ascii(mut self) -> Self {
        self.use_unicode = false;
        self
    }
}

/// Format a header with appropriate styling
pub fn format_header(text: &str, style: &OutputStyle) -> String {
    if style.use_color {
        text.bold().bright_blue().to_string()
    } else {
        text.to_string()
    }
}

/// Format a success message
pub fn format_success(text: &str, style: &OutputStyle) -> String {
    if style.use_color {
        text.green().to_string()
    } else {
        text.to_string()
    }
}

/// Format a warning message
pub fn format_warning(text: &str, style: &OutputStyle) -> String {
    if style.use_color {
        text.yellow().to_string()
    } else {
        text.to_string()
    }
}

/// Format an error message
pub fn format_error(text: &str, style: &OutputStyle) -> String {
    if style.use_color {
        text.red().to_string()
    } else {
        text.to_string()
    }
}

/// Format a key-value pair
pub fn format_key_value(key: &str, value: &str, style: &OutputStyle) -> String {
    if style.use_color {
        format!("{}: {}", key.cyan(), value)
    } else {
        format!("{}: {}", key, value)
    }
}

/// Create a styled table
pub fn create_table(style: &OutputStyle) -> Table {
    let mut table = Table::new();

    // Set table style based on preferences
    if style.use_unicode {
        table
            .load_preset(presets::UTF8_FULL)
            .apply_modifier(comfy_table::modifiers::UTF8_ROUND_CORNERS);
    } else {
        table.load_preset(presets::ASCII_FULL);
    }

    // Configure table layout
    table
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(140);

    table
}

/// Create a simple list table (no borders)
pub fn create_list_table(style: &OutputStyle) -> Table {
    let mut table = Table::new();

    if style.use_unicode {
        table.load_preset(presets::UTF8_HORIZONTAL_ONLY);
    } else {
        table.load_preset(presets::ASCII_HORIZONTAL_ONLY);
    }

    table
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(100);

    table
}

/// Style a table header cell
pub fn header_cell(text: &str, style: &OutputStyle) -> Cell {
    let cell = Cell::new(text);
    if style.use_color {
        cell.fg(Color::Cyan)
            .add_attribute(Attribute::Bold)
            .set_alignment(CellAlignment::Left)
    } else {
        cell.add_attribute(Attribute::Bold)
            .set_alignment(CellAlignment::Left)
    }
}

/// Style a regular cell
pub fn regular_cell(text: &str) -> Cell {
    Cell::new(text).set_alignment(CellAlignment::Left)
}

/// Style a numeric cell (right-aligned)
pub fn numeric_cell(text: &str) -> Cell {
    Cell::new(text).set_alignment(CellAlignment::Right)
}

/// Style a status cell based on content
pub fn status_cell(text: &str, style: &OutputStyle) -> Cell {
    let cell = Cell::new(text);
    if style.use_color {
        match text {
            "✓" | "OK" | "Success" | "Complete" => cell.fg(Color::Green),
            "✗" | "Failed" | "Error" => cell.fg(Color::Red),
            "⚠" | "Warning" => cell.fg(Color::Yellow),
            "…" | "In Progress" => cell.fg(Color::Blue),
            _ => cell,
        }
    } else {
        cell
    }
}

/// Style a hash cell (dimmed)
pub fn hash_cell(text: &str, style: &OutputStyle) -> Cell {
    let cell = Cell::new(text);
    if style.use_color {
        cell.fg(Color::Grey)
    } else {
        cell
    }
}

/// Print a section header
pub fn print_section_header(title: &str, style: &OutputStyle) {
    if style.use_color {
        println!("\n{}", title.bold().bright_blue());
        println!("{}", "═".repeat(title.len()).bright_blue());
    } else {
        println!("\n{}", title);
        println!("{}", "=".repeat(title.len()));
    }
}

/// Print a subsection header
pub fn print_subsection_header(title: &str, style: &OutputStyle) {
    if style.use_color {
        println!("\n{}", title.cyan());
        println!("{}", "─".repeat(title.len()).cyan());
    } else {
        println!("\n{}", title);
        println!("{}", "-".repeat(title.len()));
    }
}

/// Format a count badge (e.g., "(42 items)")
pub fn format_count_badge(count: usize, item_name: &str, style: &OutputStyle) -> String {
    let text = if count == 1 {
        format!("({} {})", count, item_name)
    } else {
        format!("({} {}s)", count, item_name)
    };

    if style.use_color {
        text.dimmed().to_string()
    } else {
        text
    }
}

/// Format a timestamp
pub fn format_timestamp(timestamp: &str, style: &OutputStyle) -> String {
    if style.use_color {
        timestamp.dimmed().to_string()
    } else {
        timestamp.to_string()
    }
}

/// Format a file path
pub fn format_path(path: &str, style: &OutputStyle) -> String {
    if style.use_color {
        path.bright_magenta().to_string()
    } else {
        path.to_string()
    }
}

/// Format a URL
pub fn format_url(url: &str, style: &OutputStyle) -> String {
    if style.use_color {
        url.bright_blue().underline().to_string()
    } else {
        url.to_string()
    }
}

/// Format a hash or ID
pub fn format_hash(hash: &str, style: &OutputStyle) -> String {
    if style.use_color {
        hash.dimmed().italic().to_string()
    } else {
        hash.to_string()
    }
}
