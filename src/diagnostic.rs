//! # Diagnostic Module
//!
//! Rust-style rich diagnostic system for Restrict Language.
//!
//! Provides detailed error messages with:
//! - Source code display with line numbers
//! - Underline annotations (^^^) pointing to problematic code
//! - Multiple labels per diagnostic
//! - Notes and help suggestions
//! - ANSI color support for terminal output
//!
//! ## Example Output
//!
//! ```text
//! error[E0001]: use of moved value: `x`
//!   --> src/main.rl:4:5
//!    |
//!  2 |     let x = getData();
//!    |         - move occurs because `x` has type `Data`
//!  3 |     x |> process;
//!    |     - value moved here
//!  4 |     x |> display;
//!    |     ^ value used here after move
//!    |
//! help: consider using `clone` to keep a copy
//!    |
//!  3 |     x.clone |> process;
//!    |      ++++++
//! ```

use crate::lexer::Span;
use std::fmt;
use std::io::Write;

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// An error that prevents compilation
    Error,
    /// A warning that doesn't prevent compilation
    Warning,
    /// An informational note
    Note,
    /// A help suggestion
    Help,
}

impl Severity {
    /// Returns the display name of this severity level.
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
            Severity::Help => "help",
        }
    }

    /// Returns the ANSI color code for this severity level.
    pub fn color_code(&self) -> &'static str {
        match self {
            Severity::Error => "\x1b[1;31m",    // Bold red
            Severity::Warning => "\x1b[1;33m", // Bold yellow
            Severity::Note => "\x1b[1;36m",    // Bold cyan
            Severity::Help => "\x1b[1;32m",    // Bold green
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Style of underline for a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    /// Primary label (^^^) - the main cause of the diagnostic
    Primary,
    /// Secondary label (---) - related information
    Secondary,
}

/// A label attached to a span in the source code.
#[derive(Debug, Clone)]
pub struct Label {
    /// The span this label points to
    pub span: Span,
    /// The message to display
    pub message: String,
    /// The style of this label
    pub style: LabelStyle,
}

impl Label {
    /// Creates a new primary label.
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            style: LabelStyle::Primary,
        }
    }

    /// Creates a new secondary label.
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
            style: LabelStyle::Secondary,
        }
    }
}

/// A rich diagnostic message.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity level
    pub severity: Severity,
    /// Optional error code (e.g., "E0001")
    pub code: Option<String>,
    /// Main error message
    pub message: String,
    /// Labels pointing to source locations
    pub labels: Vec<Label>,
    /// Additional notes
    pub notes: Vec<String>,
    /// Help suggestions
    pub help: Vec<String>,
    /// Optional filename for display
    pub filename: Option<String>,
}

impl Diagnostic {
    /// Creates a new error diagnostic.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            filename: None,
        }
    }

    /// Creates a new warning diagnostic.
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            filename: None,
        }
    }

    /// Creates a new note diagnostic.
    pub fn note(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Note,
            code: None,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            help: Vec::new(),
            filename: None,
        }
    }

    /// Sets the error code.
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Sets the filename.
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Adds a primary label.
    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::primary(span, message));
        self
    }

    /// Adds a secondary label.
    pub fn with_secondary_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label::secondary(span, message));
        self
    }

    /// Adds a note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Adds a help suggestion.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help.push(help.into());
        self
    }
}

/// ANSI color codes for terminal output.
pub struct Colors;

impl Colors {
    pub const RESET: &'static str = "\x1b[0m";
    pub const BOLD: &'static str = "\x1b[1m";
    pub const RED: &'static str = "\x1b[31m";
    pub const BOLD_RED: &'static str = "\x1b[1;31m";
    pub const GREEN: &'static str = "\x1b[32m";
    pub const BOLD_GREEN: &'static str = "\x1b[1;32m";
    pub const YELLOW: &'static str = "\x1b[33m";
    pub const BOLD_YELLOW: &'static str = "\x1b[1;33m";
    pub const BLUE: &'static str = "\x1b[34m";
    pub const BOLD_BLUE: &'static str = "\x1b[1;34m";
    pub const CYAN: &'static str = "\x1b[36m";
    pub const BOLD_CYAN: &'static str = "\x1b[1;36m";
    pub const WHITE: &'static str = "\x1b[37m";
    pub const BOLD_WHITE: &'static str = "\x1b[1;37m";
}

/// Configuration for diagnostic rendering.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Whether to use ANSI colors
    pub colors: bool,
    /// Number of context lines to show before/after
    pub context_lines: usize,
    /// Character to use for tab display
    pub tab_width: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            colors: true,
            context_lines: 1,
            tab_width: 4,
        }
    }
}

impl RenderConfig {
    /// Creates a config without colors (for non-terminal output).
    pub fn no_colors() -> Self {
        Self {
            colors: false,
            ..Default::default()
        }
    }
}

/// Renders diagnostics to a string or writer.
pub struct DiagnosticRenderer {
    config: RenderConfig,
}

impl DiagnosticRenderer {
    /// Creates a new renderer with the given configuration.
    pub fn new(config: RenderConfig) -> Self {
        Self { config }
    }

    /// Creates a renderer with default colored output.
    pub fn colored() -> Self {
        Self::new(RenderConfig::default())
    }

    /// Creates a renderer without colors.
    pub fn plain() -> Self {
        Self::new(RenderConfig::no_colors())
    }

    /// Renders a diagnostic to a string.
    pub fn render(&self, diagnostic: &Diagnostic, source: &str) -> String {
        let mut output = String::new();
        self.render_to(&mut output, diagnostic, source);
        output
    }

    /// Renders a diagnostic to a writer.
    pub fn render_to(&self, output: &mut dyn fmt::Write, diagnostic: &Diagnostic, source: &str) {
        let _ = self.render_header(output, diagnostic);
        let _ = self.render_labels(output, diagnostic, source);
        let _ = self.render_notes(output, diagnostic);
        let _ = self.render_help(output, diagnostic);
    }

    /// Renders a diagnostic to an io::Write (e.g., stderr).
    pub fn render_to_io<W: Write>(&self, output: &mut W, diagnostic: &Diagnostic, source: &str) -> std::io::Result<()> {
        let rendered = self.render(diagnostic, source);
        output.write_all(rendered.as_bytes())
    }

    fn color<'a>(&self, code: &'a str) -> &'a str {
        if self.config.colors { code } else { "" }
    }

    fn reset(&self) -> &str {
        if self.config.colors { Colors::RESET } else { "" }
    }

    fn render_header(&self, output: &mut dyn fmt::Write, diagnostic: &Diagnostic) -> fmt::Result {
        let severity_color = diagnostic.severity.color_code();

        write!(output, "{}{}", self.color(severity_color), diagnostic.severity)?;

        if let Some(code) = &diagnostic.code {
            write!(output, "[{}]", code)?;
        }

        write!(output, "{}: {}{}{}\n",
            self.reset(),
            self.color(Colors::BOLD_WHITE),
            diagnostic.message,
            self.reset()
        )
    }

    fn render_labels(&self, output: &mut dyn fmt::Write, diagnostic: &Diagnostic, source: &str) -> fmt::Result {
        if diagnostic.labels.is_empty() {
            return Ok(());
        }

        // Sort labels by span start
        let mut labels = diagnostic.labels.clone();
        labels.sort_by_key(|l| l.span.start);

        // Get line information for each label
        let lines: Vec<&str> = source.lines().collect();
        let line_starts: Vec<usize> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();

        // Find which lines we need to display
        let mut lines_to_show: Vec<usize> = Vec::new();
        for label in &labels {
            let (line, _col) = label.span.to_line_col(source);
            for i in line.saturating_sub(self.config.context_lines)..=line + self.config.context_lines {
                if i < lines.len() && !lines_to_show.contains(&i) {
                    lines_to_show.push(i);
                }
            }
        }
        lines_to_show.sort();
        lines_to_show.dedup();

        // Get max line number width for padding
        let max_line_num = lines_to_show.iter().max().map(|n| n + 1).unwrap_or(1);
        let line_num_width = format!("{}", max_line_num).len();

        // Render location header
        if let Some(first_label) = labels.first() {
            let (line, col) = first_label.span.to_line_col(source);
            let filename = diagnostic.filename.as_deref().unwrap_or("<source>");
            write!(output, "{}{:>width$}--> {}{}:{}:{}\n",
                self.color(Colors::BOLD_BLUE),
                "",
                self.reset(),
                filename,
                line + 1,
                col + 1,
                width = line_num_width
            )?;
        }

        // Render blank separator line
        write!(output, "{}{:>width$} |\n",
            self.color(Colors::BOLD_BLUE),
            "",
            width = line_num_width
        )?;

        // Render each line with labels
        let mut prev_line: Option<usize> = None;
        for &line_idx in &lines_to_show {
            // Check if we need to show a gap indicator
            if let Some(prev) = prev_line {
                if line_idx > prev + 1 {
                    write!(output, "{}...{}\n", self.color(Colors::BOLD_BLUE), self.reset())?;
                }
            }
            prev_line = Some(line_idx);

            let line_content = lines.get(line_idx).unwrap_or(&"");
            let line_start = line_starts.get(line_idx).copied().unwrap_or(0);
            let line_end = line_start + line_content.len();

            // Render line number and content
            write!(output, "{}{:>width$} |{} {}\n",
                self.color(Colors::BOLD_BLUE),
                line_idx + 1,
                self.reset(),
                line_content,
                width = line_num_width
            )?;

            // Render underlines for labels on this line
            for label in &labels {
                let (label_line, label_col) = label.span.to_line_col(source);
                if label_line != line_idx {
                    continue;
                }

                // Calculate underline position and length
                let span_start_in_line = label.span.start.saturating_sub(line_start);
                let span_end_in_line = label.span.end.min(line_end).saturating_sub(line_start);
                let underline_len = span_end_in_line.saturating_sub(span_start_in_line).max(1);

                // Render underline
                let underline_char = match label.style {
                    LabelStyle::Primary => '^',
                    LabelStyle::Secondary => '-',
                };
                let underline_color = match label.style {
                    LabelStyle::Primary => diagnostic.severity.color_code(),
                    LabelStyle::Secondary => Colors::BOLD_BLUE,
                };

                let padding = " ".repeat(span_start_in_line);
                let underline = underline_char.to_string().repeat(underline_len);

                write!(output, "{}{:>width$} |{} {}{}{}",
                    self.color(Colors::BOLD_BLUE),
                    "",
                    self.reset(),
                    padding,
                    self.color(underline_color),
                    underline,
                    width = line_num_width
                )?;

                // Add label message if present
                if !label.message.is_empty() {
                    write!(output, " {}", label.message)?;
                }
                write!(output, "{}\n", self.reset())?;
            }
        }

        // Render closing separator
        write!(output, "{}{:>width$} |{}\n",
            self.color(Colors::BOLD_BLUE),
            "",
            self.reset(),
            width = line_num_width
        )
    }

    fn render_notes(&self, output: &mut dyn fmt::Write, diagnostic: &Diagnostic) -> fmt::Result {
        for note in &diagnostic.notes {
            write!(output, "{} = {}note{}: {}\n",
                self.color(Colors::BOLD_BLUE),
                self.color(Colors::BOLD_CYAN),
                self.reset(),
                note
            )?;
        }
        Ok(())
    }

    fn render_help(&self, output: &mut dyn fmt::Write, diagnostic: &Diagnostic) -> fmt::Result {
        for help in &diagnostic.help {
            write!(output, "{} = {}help{}: {}\n",
                self.color(Colors::BOLD_BLUE),
                self.color(Colors::BOLD_GREEN),
                self.reset(),
                help
            )?;
        }
        Ok(())
    }
}

/// A collection of diagnostics.
#[derive(Debug, Default)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    /// Creates a new empty diagnostic bag.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a diagnostic.
    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Adds an error.
    pub fn error(&mut self, message: impl Into<String>) -> &mut Diagnostic {
        self.diagnostics.push(Diagnostic::error(message));
        self.diagnostics.last_mut().unwrap()
    }

    /// Adds a warning.
    pub fn warning(&mut self, message: impl Into<String>) -> &mut Diagnostic {
        self.diagnostics.push(Diagnostic::warning(message));
        self.diagnostics.last_mut().unwrap()
    }

    /// Returns true if there are any errors.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    /// Returns the number of errors.
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    /// Returns the number of warnings.
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning).count()
    }

    /// Returns all diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Consumes the bag and returns all diagnostics.
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Renders all diagnostics to a string.
    pub fn render(&self, source: &str, config: RenderConfig) -> String {
        let use_colors = config.colors;
        let renderer = DiagnosticRenderer::new(config);
        let mut output = String::new();
        for diagnostic in &self.diagnostics {
            output.push_str(&renderer.render(diagnostic, source));
            output.push('\n');
        }

        // Add summary
        let errors = self.error_count();
        let warnings = self.warning_count();
        if errors > 0 || warnings > 0 {
            if errors > 0 {
                output.push_str(&format!("{}error{}: aborting due to {} previous error{}\n",
                    if use_colors { Colors::BOLD_RED } else { "" },
                    if use_colors { Colors::RESET } else { "" },
                    errors,
                    if errors == 1 { "" } else { "s" }
                ));
            }
            if warnings > 0 {
                output.push_str(&format!("{}warning{}: {} warning{} emitted\n",
                    if use_colors { Colors::BOLD_YELLOW } else { "" },
                    if use_colors { Colors::RESET } else { "" },
                    warnings,
                    if warnings == 1 { "" } else { "s" }
                ));
            }
        }

        output
    }

    /// Renders all diagnostics to stderr.
    pub fn emit_to_stderr(&self, source: &str) {
        let config = RenderConfig::default();
        let output = self.render(source, config);
        eprint!("{}", output);
    }
}

/// LSP integration - only available when tower-lsp is available (non-WASM)
#[cfg(not(target_arch = "wasm32"))]
pub mod lsp_integration {
    use super::*;
    use tower_lsp::lsp_types::{
        Diagnostic as LspDiagnostic,
        DiagnosticSeverity,
        DiagnosticRelatedInformation,
        Position,
        Range,
        Location,
        Url,
        NumberOrString,
    };

    impl Severity {
        /// Converts to LSP DiagnosticSeverity.
        pub fn to_lsp(&self) -> DiagnosticSeverity {
            match self {
                Severity::Error => DiagnosticSeverity::ERROR,
                Severity::Warning => DiagnosticSeverity::WARNING,
                Severity::Note => DiagnosticSeverity::INFORMATION,
                Severity::Help => DiagnosticSeverity::HINT,
            }
        }
    }

    impl Diagnostic {
        /// Converts this diagnostic to an LSP Diagnostic.
        ///
        /// # Arguments
        /// * `source` - The source code for span-to-position conversion
        /// * `uri` - The document URI for related information
        pub fn to_lsp(&self, source: &str, uri: &Url) -> LspDiagnostic {
            // Get primary label span or default to start of file
            let range = if let Some(label) = self.labels.first() {
                span_to_range(source, &label.span)
            } else {
                Range::new(Position::new(0, 0), Position::new(0, 1))
            };

            // Build related information from secondary labels
            let related_information: Option<Vec<DiagnosticRelatedInformation>> = {
                let related: Vec<_> = self.labels.iter()
                    .filter(|l| l.style == super::LabelStyle::Secondary && !l.message.is_empty())
                    .map(|label| DiagnosticRelatedInformation {
                        location: Location {
                            uri: uri.clone(),
                            range: span_to_range(source, &label.span),
                        },
                        message: label.message.clone(),
                    })
                    .collect();
                if related.is_empty() { None } else { Some(related) }
            };

            // Build full message with notes and help
            let mut full_message = self.message.clone();
            for note in &self.notes {
                full_message.push_str(&format!("\nnote: {}", note));
            }
            for help in &self.help {
                full_message.push_str(&format!("\nhelp: {}", help));
            }

            LspDiagnostic {
                range,
                severity: Some(self.severity.to_lsp()),
                code: self.code.clone().map(NumberOrString::String),
                code_description: None,
                source: Some("restrict-lang".to_string()),
                message: full_message,
                related_information,
                tags: None,
                data: None,
            }
        }
    }

    impl DiagnosticBag {
        /// Converts all diagnostics to LSP Diagnostics.
        pub fn to_lsp(&self, source: &str, uri: &Url) -> Vec<LspDiagnostic> {
            self.diagnostics.iter()
                .map(|d| d.to_lsp(source, uri))
                .collect()
        }
    }

    /// Helper function to convert a Span to an LSP Range.
    pub fn span_to_range(source: &str, span: &Span) -> Range {
        let (start_line, start_col) = span.to_line_col(source);
        let end_span = Span::new(span.end, span.end);
        let (end_line, end_col) = end_span.to_line_col(source);

        Range::new(
            Position::new(start_line as u32, start_col as u32),
            Position::new(end_line as u32, end_col as u32),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_builder() {
        let diag = Diagnostic::error("undefined variable")
            .with_code("E0001")
            .with_filename("test.rl")
            .with_label(Span::new(10, 15), "not found in this scope")
            .with_note("variables must be declared before use")
            .with_help("did you mean `count`?");

        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.code, Some("E0001".to_string()));
        assert_eq!(diag.message, "undefined variable");
        assert_eq!(diag.labels.len(), 1);
        assert_eq!(diag.notes.len(), 1);
        assert_eq!(diag.help.len(), 1);
    }

    #[test]
    fn test_render_simple_error() {
        let source = "let x = 42;\nlet y = x + count;";
        let diag = Diagnostic::error("undefined variable: `count`")
            .with_code("E0001")
            .with_label(Span::new(24, 29), "not found in this scope");

        let renderer = DiagnosticRenderer::plain();
        let output = renderer.render(&diag, source);

        assert!(output.contains("error[E0001]"));
        assert!(output.contains("undefined variable"));
        assert!(output.contains("not found in this scope"));
    }

    #[test]
    fn test_render_multiple_labels() {
        let source = "let x = getData();\nx |> process;\nx |> display;";
        let diag = Diagnostic::error("use of moved value: `x`")
            .with_code("E0002")
            .with_secondary_label(Span::new(4, 5), "move occurs here")
            .with_secondary_label(Span::new(19, 20), "value moved here")
            .with_label(Span::new(33, 34), "value used here after move")
            .with_help("consider using `clone` if you need multiple copies");

        let renderer = DiagnosticRenderer::plain();
        let output = renderer.render(&diag, source);

        assert!(output.contains("error[E0002]"));
        assert!(output.contains("use of moved value"));
    }

    #[test]
    fn test_diagnostic_bag() {
        let mut bag = DiagnosticBag::new();
        bag.add(Diagnostic::error("error 1"));
        bag.add(Diagnostic::warning("warning 1"));
        bag.add(Diagnostic::error("error 2"));

        assert!(bag.has_errors());
        assert_eq!(bag.error_count(), 2);
        assert_eq!(bag.warning_count(), 1);
    }
}
