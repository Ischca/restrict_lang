//! User-facing diagnostic formatting helpers.

use crate::type_checker::TypeError;

pub type NomError<'a> = nom::Err<nom::error::Error<&'a str>>;

pub fn format_lex_error(source: &str, error: NomError<'_>) -> String {
    format_nom_error("Lexing", source, error)
}

pub fn format_parse_error(source: &str, error: NomError<'_>) -> String {
    format_nom_error("Parsing", source, error)
}

pub fn format_unparsed_input(source: &str, remaining: &str) -> String {
    if !is_slice_from_source(source, remaining) {
        return format!(
            "Parsing error: unparsed input near `{}`",
            truncate_for_diagnostic(remaining.trim(), 48)
        );
    }

    let leading_whitespace = remaining.len().saturating_sub(remaining.trim_start().len());
    let offset = source
        .len()
        .saturating_sub(remaining.len())
        .saturating_add(leading_whitespace);
    let (line, column) = line_column(source, offset);
    let near = remaining.trim_start().lines().next().unwrap_or("");
    let mut message = format!(
        "Parsing error at line {line}, column {column}: unparsed input near `{}`",
        truncate_for_diagnostic(near, 48)
    );
    if let Some((source_line, caret_column)) = source_line_at(source, offset) {
        message.push_str(&format!(
            "\n  {source_line}\n  {}^ parsing stopped here",
            " ".repeat(caret_column.saturating_sub(1))
        ));
    }
    message
}

pub fn format_type_error(source: &str, error: &TypeError) -> String {
    let mut message = format!("Type error: {error}");
    let TypeError::NonCallableVerb { name, .. } = error else {
        return message;
    };
    let Some((line, column, source_line)) = find_osv_verb(source, name) else {
        return message;
    };
    message.push_str(&format!(
        "\n  at line {line}, column {column}\n  {source_line}\n  {}^ non-callable OSV verb",
        " ".repeat(column.saturating_sub(1))
    ));
    message
}

fn find_osv_verb<'a>(source: &'a str, name: &str) -> Option<(usize, usize, &'a str)> {
    let mut previous_significant_line: Option<&str> = None;
    let mut candidate = None;
    for (line_index, line) in source.lines().enumerate() {
        for (byte_index, _) in line.match_indices(name) {
            let before_boundary = line[..byte_index]
                .chars()
                .next_back()
                .is_none_or(|ch| !is_identifier_continue(ch));
            let after_index = byte_index + name.len();
            let after_boundary = line[after_index..]
                .chars()
                .next()
                .is_none_or(|ch| !is_identifier_continue(ch));
            if !before_boundary || !after_boundary {
                continue;
            }

            let before = line[..byte_index].trim();
            let after = line[after_index..].trim_start();
            if after.starts_with('=') || after.starts_with(':') {
                continue;
            }
            let same_line_object = !before.is_empty();
            let continued_object = before.is_empty()
                && previous_significant_line
                    .is_some_and(|previous| !previous.ends_with(['{', '=', ';', ',']));
            let followed_by_another_identifier =
                after.chars().next().is_some_and(is_identifier_continue);
            if (same_line_object || continued_object) && !followed_by_another_identifier {
                let column = line[..byte_index].chars().count().saturating_add(1);
                candidate = Some((line_index + 1, column, line));
            }
        }

        let significant = line.split("//").next().unwrap_or("").trim();
        if !significant.is_empty() {
            previous_significant_line = Some(significant);
        }
    }
    candidate
}

fn is_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
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
                let mut message = format!(
                    "{phase} error at line {line}, column {column}: unexpected input near `{}`",
                    truncate_for_diagnostic(near, 48)
                );
                if let Some((source_line, caret_column)) = source_line_at(source, offset) {
                    message.push_str(&format!(
                        "\n  {source_line}\n  {}^ parsing stopped here",
                        " ".repeat(caret_column.saturating_sub(1))
                    ));
                }
                message
            }
        }
        nom::Err::Incomplete(_) => format!("{phase} error: incomplete input"),
    }
}

fn source_line_at(source: &str, offset: usize) -> Option<(&str, usize)> {
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = source[offset..]
        .find('\n')
        .map_or(source.len(), |index| offset + index);
    let source_line = source.get(line_start..line_end)?;
    let caret_column = source
        .get(line_start..offset)?
        .chars()
        .count()
        .saturating_add(1);
    Some((source_line, caret_column))
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

    let column = source
        .get(line_start..offset)
        .map_or(1, |prefix| prefix.chars().count().saturating_add(1));
    (line, column)
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
        assert!(message.contains("val answer ="));
        assert!(message.contains("^ parsing stopped here"));
    }

    #[test]
    fn function_body_error_points_inside_the_body_instead_of_at_fun() {
        let source = r#"record Quote {
    total: Int32
}

fun make_quote: () -> Quote = {
    val total = 42;
    Quote { total }
}
"#;
        let err = parse_program(source).expect_err("record shorthand is not supported");
        let message = format_parse_error(source, err);

        assert!(message.contains("line 7"), "{message}");
        assert!(message.contains("Quote { total }"), "{message}");
        assert!(!message.contains("line 5, column 1"), "{message}");
    }

    #[test]
    fn unparsed_input_uses_character_columns_and_a_source_caret() {
        let source = "fun main: () -> () = {\n    \"日本語\"\n    ???\n}\n";
        let offset = source.find("???").unwrap();
        let message = format_unparsed_input(source, &source[offset..]);

        assert!(message.contains("line 3, column 5"), "{message}");
        assert!(message.contains("    ???"), "{message}");
        assert!(message.contains("    ^ parsing stopped here"), "{message}");
    }

    #[test]
    fn non_callable_verb_diagnostic_points_to_the_ambiguous_continuation() {
        let source = r#"fun main: () -> Int32 = {
    val first = 1;
    val second = 2;
    first
    second
}
"#;
        let error = TypeError::NonCallableVerb {
            name: "second".to_string(),
            found: "Int32".to_string(),
        };
        let message = format_type_error(source, &error);

        assert!(message.contains("at line 5, column 5"), "{message}");
        assert!(message.contains("^ non-callable OSV verb"), "{message}");
    }

    #[test]
    fn non_callable_verb_after_a_completed_block_wins_over_rhs_uses() {
        let source = r#"fun benchmark: (iterations: Int32) -> Int32 = {
    mut val index = 0;
    mut val checksum = 0;
    (index < iterations) while {
        checksum = checksum + 1;
        index = index + 1
    }
    checksum
}
"#;
        let error = TypeError::NonCallableVerb {
            name: "checksum".to_string(),
            found: "Int32".to_string(),
        };
        let message = format_type_error(source, &error);

        assert!(message.contains("at line 8, column 5"), "{message}");
        assert!(message.contains("    checksum"), "{message}");
    }

    #[test]
    fn parse_error_formatter_preserves_unsupported_feature_messages() {
        let source = r#"form Printable<T> {
    fun display: (self: Self) -> String
}
"#;
        let err = parse_program(source).expect_err("generic forms are intentionally deferred");
        let message = format_parse_error(source, err);

        assert!(message.contains("generic forms are not supported yet"));
        assert!(message.contains("declare a non-generic form"));
        assert!(!message.contains("unexpected input near"));
        assert!(!message.contains("Error("));
        assert!(!message.contains("ErrorKind"));
    }
}
