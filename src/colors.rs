use console::Style;

/// Catppuccin Mocha palette mapped to 256-color terminal values.
///
/// The `console` crate only supports `Color256(u8)`, not true 24-bit RGB,
/// so these are the closest xterm-256 approximations of each Mocha hex value.
pub struct Colors {
    /// #fab387 — warnings
    pub peach: Style,
    /// #a6e3a1 — success
    pub green: Style,
    /// #f9e2af — highlights
    pub yellow: Style,
    /// #74c7ec — dir names, note titles
    pub sapphire: Style,
    /// #b4befe — headers
    pub lavender: Style,
    /// #cba6f7 — section labels
    pub mauve: Style,
    /// #7f849c — subtle text, paths
    pub overlay: Style,
    /// #45475a — dividers, dots
    pub surface: Style,
    /// Bold (no color)
    pub bold: Style,
}

impl Colors {
    pub fn new() -> Self {
        Self {
            peach: Style::new().color256(216),
            green: Style::new().color256(150),
            yellow: Style::new().color256(223),
            sapphire: Style::new().color256(117),
            lavender: Style::new().color256(147),
            mauve: Style::new().color256(183),
            overlay: Style::new().color256(102),
            surface: Style::new().color256(59),
            bold: Style::new().bold(),
        }
    }

    /// Full-width divider line in surface color.
    pub fn divider(&self, width: usize) -> String {
        self.surface.apply_to("─".repeat(width)).to_string()
    }

    /// Section header: `  ── Label ────────…` filling `width` columns.
    pub fn section(&self, label: &str, width: usize) -> String {
        let prefix = format!("── {} ", label);
        let remain = width.saturating_sub(prefix.len());
        format!(
            "  {}── {}{} {}",
            self.surface.apply_to(""),
            self.mauve.apply_to(label),
            self.surface.apply_to(" "),
            self.surface.apply_to("─".repeat(remain))
        )
    }
}
