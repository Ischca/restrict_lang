//! User-facing diagnostic formatting helpers.

pub type NomError<'a> = nom::Err<nom::error::Error<&'a str>>;

pub fn format_lex_error(source: &str, error: NomError<'_>) -> String {
    format_nom_error("Lexing", source, error)
}

pub fn format_parse_error(source: &str, error: NomError<'_>) -> String {
    format_nom_error("Parsing", source, error)
}

fn format_nom_error(phase: &str, source: &str, error: NomError<'_>) -> String {
    match error {
        nom::Err::Error(error) | nom::Err::Failure(error) => {
            if !error.input.is_empty() && !is_slice_from_source(source, error.input) {
                return format!(
                    "{phase} error: {}",
                    truncate_for_diagnostic(error.input.trim(), 120)
                );
            }

            let offset = source.len().saturating_sub(error.input.len());
            let (line, column) = line_column(source, offset);
            let near = error.input.trim().lines().next().unwrap_or("");
            if near.is_empty() {
                format!("{phase} error at line {line}, column {column}: unexpected end of input")
            } else {
                format!(
                    "{phase} error at line {line}, column {column}: unexpected input near `{}`",
                    truncate_for_diagnostic(near, 48)
                )
            }
        }
        nom::Err::Incomplete(_) => format!("{phase} error: incomplete input"),
    }
}

fn is_slice_from_source(source: &str, input: &str) -> bool {
    let source_start = source.as_ptr() as usize;
    let source_end = source_start.saturating_add(source.len());
    let input_start = input.as_ptr() as usize;
    let input_end = input_start.saturating_add(input.len());

    input_start >= source_start && input_end <= source_end
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let mut line = 1usize;
    let mut line_start = 0usize;

    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }

    (line, offset.saturating_sub(line_start) + 1)
}

fn truncate_for_diagnostic(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let mut output = String::new();
    for _ in 0..max_chars {
        match chars.next() {
            Some(ch) => output.push(ch),
            None => return output,
        }
    }
    if chars.next().is_some() {
        output.push_str("...");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_program;

    #[test]
    fn parse_error_formatter_uses_line_column_without_nom_debug() {
        let source = "fun main: () -> Int32 = {\n    val answer =\n}\n";
        let err = parse_program(source).expect_err("source should not parse");
        let message = format_parse_error(source, err);

        assert!(message.contains("Parsing error at line"));
        assert!(message.contains("column"));
        assert!(!message.contains("Error("));
        assert!(!message.contains("ErrorKind"));
        assert!(!message.contains("nom"));
    }

    #[test]
    fn parse_error_formatter_preserves_unsupported_feature_messages() {
        let source = "enum ReviewState { Ready }\n";
        let err = parse_program(source).expect_err("enum declarations are not implemented");
        let message = format_parse_error(source, err);

        assert!(message.contains("enum declarations are unsupported in v0.0.1"));
        assert!(message.contains("user-defined enum declarations are not implemented"));
        assert!(!message.contains("unexpected input near"));
        assert!(!message.contains("Error("));
        assert!(!message.contains("ErrorKind"));
    }
}
