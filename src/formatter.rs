//! Token- and comment-preserving source formatter for Restrict Language.

use crate::lexer::{lex_token, lex_tokens, Token};

#[derive(Debug, Clone, PartialEq)]
enum Item<'a> {
    Token {
        token: Token,
        raw: &'a str,
        newlines_before: usize,
        attached_next: bool,
    },
    LineComment {
        raw: &'a str,
        newlines_before: usize,
    },
    BlockComment {
        raw: &'a str,
        newlines_before: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delim {
    Brace,
    Paren,
    Bracket,
}

/// Formats Restrict source code canonically while preserving all non-whitespace
/// tokens and comments.
///
/// Enforces a post-format invariant check ensuring the lexed token stream and
/// comment sequence are unchanged.
pub fn format_source(source: &str) -> Result<String, String> {
    let items = scan_items(source)?;
    if items.is_empty() {
        return Ok(String::new());
    }

    let generic_lt_indices = find_generic_angle_brackets(&items);
    let mut generic_gt_indices = vec![false; items.len()];
    let mut angle_depth = 0usize;
    for (idx, item) in items.iter().enumerate() {
        if let Item::Token { token, .. } = item {
            if generic_lt_indices[idx] {
                angle_depth += 1;
            } else if matches!(token, Token::Gt) && angle_depth > 0 {
                generic_gt_indices[idx] = true;
                angle_depth -= 1;
            }
        }
    }

    // Group items into physical lines while preserving user line-break intent.
    let mut lines: Vec<(usize, Vec<usize>)> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        let newlines_before = match item {
            Item::Token {
                newlines_before, ..
            }
            | Item::LineComment {
                newlines_before, ..
            }
            | Item::BlockComment {
                newlines_before, ..
            } => *newlines_before,
        };
        if lines.is_empty() || newlines_before > 0 {
            let blank_lines = if lines.is_empty() {
                0
            } else {
                usize::from(newlines_before >= 2)
            };
            lines.push((blank_lines, vec![idx]));
        } else if let Some((_, current_line)) = lines.last_mut() {
            current_line.push(idx);
        }
    }

    let mut output = String::with_capacity(source.len());
    let mut delim_stack: Vec<Delim> = Vec::new();
    let mut lambda_bar_depth: Option<usize> = None;
    let mut in_import_line = false;

    for (line_pos, (blank_lines, line_indices)) in lines.iter().enumerate() {
        if line_pos > 0 {
            output.push('\n');
            for _ in 0..*blank_lines {
                output.push('\n');
            }
        }

        // Pop leading closing delimiters from the line's indent level.
        let mut leading_closers = 0usize;
        for &idx in line_indices {
            match &items[idx] {
                Item::Token {
                    token: Token::RBrace | Token::RParen | Token::RBracket,
                    ..
                } => leading_closers += 1,
                _ => break,
            }
        }
        let indent_levels = delim_stack.len().saturating_sub(leading_closers);
        output.push_str(&"    ".repeat(indent_levels));

        let mut prev_idx_on_line: Option<usize> = None;
        let mut prev_non_comment_idx: Option<usize> =
            prev_non_comment_before(&items, line_indices[0]);

        for &idx in line_indices {
            match &items[idx] {
                Item::LineComment { raw, .. } | Item::BlockComment { raw, .. } => {
                    if prev_idx_on_line.is_some() {
                        output.push(' ');
                    }
                    output.push_str(raw.trim_end());
                    prev_idx_on_line = Some(idx);
                }
                Item::Token { token, raw, .. } => {
                    if matches!(token, Token::Import) {
                        in_import_line = true;
                    }

                    // Determine lambda param bar state (`{ |x| ... }` vs `[h | t]`).
                    let mut bar_closes_lambda = false;
                    let mut bar_opens_lambda = false;
                    if matches!(token, Token::Bar) {
                        if let Some(target_depth) = lambda_bar_depth {
                            if delim_stack.len() == target_depth {
                                bar_closes_lambda = true;
                                lambda_bar_depth = None;
                            }
                        } else if prev_non_comment_idx.is_some_and(|p| {
                            matches!(
                                &items[p],
                                Item::Token {
                                    token: Token::LBrace,
                                    ..
                                }
                            )
                        }) {
                            bar_opens_lambda = true;
                            lambda_bar_depth = Some(delim_stack.len());
                        }
                    }

                    if let Some(prev_on_line) = prev_idx_on_line {
                        let space = should_insert_space(
                            &items,
                            prev_on_line,
                            idx,
                            prev_non_comment_idx,
                            &delim_stack,
                            &generic_lt_indices,
                            &generic_gt_indices,
                            lambda_bar_depth.is_some() && !bar_opens_lambda,
                            bar_closes_lambda,
                            in_import_line,
                        );
                        if space {
                            output.push(' ');
                        }
                    }

                    output.push_str(raw);

                    match token {
                        Token::LBrace => delim_stack.push(Delim::Brace),
                        Token::LParen => delim_stack.push(Delim::Paren),
                        Token::LBracket => delim_stack.push(Delim::Bracket),
                        Token::RBrace => {
                            if let Some(pos) = delim_stack.iter().rposition(|d| *d == Delim::Brace)
                            {
                                delim_stack.truncate(pos);
                            }
                            in_import_line = false;
                        }
                        Token::RParen => {
                            if let Some(pos) = delim_stack.iter().rposition(|d| *d == Delim::Paren)
                            {
                                delim_stack.truncate(pos);
                            }
                        }
                        Token::RBracket => {
                            if let Some(pos) =
                                delim_stack.iter().rposition(|d| *d == Delim::Bracket)
                            {
                                delim_stack.truncate(pos);
                            }
                        }
                        _ => {}
                    }

                    prev_idx_on_line = Some(idx);
                    prev_non_comment_idx = Some(idx);
                }
            }
        }
        in_import_line = false;
    }

    output.push('\n');

    verify_formatting_invariant(source, &output, &items)?;
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn should_insert_space(
    items: &[Item<'_>],
    prev_idx: usize,
    curr_idx: usize,
    prev_non_comment_idx: Option<usize>,
    delim_stack: &[Delim],
    generic_lt: &[bool],
    generic_gt: &[bool],
    inside_lambda_params: bool,
    bar_closes_lambda: bool,
    in_import_line: bool,
) -> bool {
    let prev_raw = item_raw(&items[prev_idx]);
    let curr_raw = item_raw(&items[curr_idx]);

    let Item::Token { token: curr, .. } = &items[curr_idx] else {
        return true;
    };
    let Item::Token { token: prev, .. } = &items[prev_idx] else {
        return true;
    };

    if would_merge_tokens(prev_raw, curr_raw) {
        return true;
    }

    // Never insert spaces around `.` or `::` or `..` or after `...` / `~` / `!`.
    if matches!(
        prev,
        Token::Dot
            | Token::ColonColon
            | Token::DotDot
            | Token::DotDotDot
            | Token::Tilde
            | Token::Not
    ) {
        return false;
    }
    if matches!(curr, Token::Dot | Token::ColonColon | Token::DotDot) {
        return false;
    }

    // `*` after `.` (`import foo.*`).
    if matches!(curr, Token::Star | Token::Asterisk) && matches!(prev, Token::Dot) {
        return false;
    }

    // Unary `-`: no space after `-` when `-` is unary.
    if matches!(prev, Token::Minus) && is_unary_minus_at(items, prev_idx) {
        return false;
    }

    // Generic `<...>` spacing.
    if generic_lt[prev_idx] || generic_gt[curr_idx] {
        return false;
    }
    if generic_lt[curr_idx] {
        // Space after `:` in `fun id: <T>(x: T)`, no space after `Type<T>`.
        return matches!(prev, Token::Colon);
    }
    if generic_gt[prev_idx] {
        // `List<Option<Int32>>` or `fun id: <T>(x: T)`: no space before `>` or `(`,
        // space before identifier/arrow/`=`/`{`/`where`.
        return !matches!(
            curr,
            Token::LParen | Token::RParen | Token::Comma | Token::Gt | Token::Colon
        );
    }

    // No space before `,`, `;`, `:`, `)`, `]`.
    if matches!(
        curr,
        Token::Comma | Token::Semicolon | Token::Colon | Token::RParen | Token::RBracket
    ) {
        return false;
    }

    // No space after `(` or `[`.
    if matches!(prev, Token::LParen | Token::LBracket) {
        return false;
    }

    // Lambda parameter bars `{ |x: Int32| ... }`.
    if bar_closes_lambda {
        return false;
    }
    if matches!(prev, Token::Bar) && inside_lambda_params {
        return false;
    }
    if matches!(prev, Token::Bar)
        && prev_non_comment_idx.is_some_and(|p| {
            let _ = p;
            false
        })
    {
        return false;
    }

    // Braces on a single line.
    if matches!(prev, Token::LBrace) && matches!(curr, Token::RBrace) {
        return false;
    }
    if matches!(prev, Token::LBrace) {
        return !in_import_line;
    }
    if matches!(curr, Token::RBrace) {
        return !in_import_line;
    }

    // Space before `{`.
    if matches!(curr, Token::LBrace) {
        return !matches!(
            prev,
            Token::Dot | Token::LParen | Token::LBracket | Token::LBrace
        );
    }

    // `~` after `<` or `(` or `,`.
    if matches!(curr, Token::Tilde) && matches!(prev, Token::Lt | Token::LParen) {
        return false;
    }

    // Inside `[ ... ]`, `,` has a trailing space and `|` in `[h | t]` has spaces around it.
    let _ = delim_stack;

    true
}

fn is_unary_minus_at(items: &[Item<'_>], minus_idx: usize) -> bool {
    if matches!(
        &items[minus_idx],
        Item::Token {
            newlines_before,
            attached_next: true,
            ..
        } if *newlines_before > 0
    ) {
        return true;
    }
    let Some(prev_idx) = prev_non_comment_before(items, minus_idx) else {
        return true;
    };
    let Item::Token { token: prev, .. } = &items[prev_idx] else {
        return true;
    };
    matches!(
        prev,
        Token::Assign
            | Token::Arrow
            | Token::ThinArrow
            | Token::Pipe
            | Token::Fallback
            | Token::Bar
            | Token::LParen
            | Token::LBracket
            | Token::LBrace
            | Token::Comma
            | Token::Colon
            | Token::Semicolon
            | Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Asterisk
            | Token::Slash
            | Token::Percent
            | Token::Eq
            | Token::Ne
            | Token::Lt
            | Token::Le
            | Token::Gt
            | Token::Ge
            | Token::And
            | Token::Or
            | Token::Not
            | Token::Then
            | Token::Else
            | Token::While
            | Token::Return
            | Token::Match
    )
}

fn find_generic_angle_brackets(items: &[Item<'_>]) -> Vec<bool> {
    let mut is_generic_lt = vec![false; items.len()];
    for (idx, item) in items.iter().enumerate() {
        let Item::Token {
            token: Token::Lt, ..
        } = item
        else {
            continue;
        };
        let Some(prev_idx) = prev_non_comment_before(items, idx) else {
            continue;
        };
        let starts_generic_owner = match &items[prev_idx] {
            Item::Token {
                token: Token::Colon | Token::Lifetime,
                ..
            } => true,
            Item::Token {
                token: Token::Ident(name),
                ..
            } => is_likely_type_or_decl_name(items, prev_idx, name),
            _ => false,
        };
        if !starts_generic_owner {
            continue;
        }
        if generic_angle_closes_cleanly(items, idx) {
            is_generic_lt[idx] = true;
        }
    }
    is_generic_lt
}

fn is_likely_type_or_decl_name(items: &[Item<'_>], ident_idx: usize, name: &str) -> bool {
    if let Some(before_ident) = prev_non_comment_before(items, ident_idx) {
        if matches!(
            &items[before_ident],
            Item::Token {
                token: Token::Record | Token::Impl | Token::Form | Token::Takes,
                ..
            }
        ) {
            return true;
        }
    }
    let starts_upper = name.chars().next().is_some_and(|c| c.is_ascii_uppercase());
    let has_lower = name.chars().any(|c| c.is_ascii_lowercase());
    starts_upper && has_lower
}

fn generic_angle_closes_cleanly(items: &[Item<'_>], lt_idx: usize) -> bool {
    let mut depth = 0usize;
    for item in &items[lt_idx..] {
        let Item::Token { token, .. } = item else {
            continue;
        };
        match token {
            Token::Lt => depth += 1,
            Token::Gt => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return true;
                }
            }
            Token::Semicolon
            | Token::LBrace
            | Token::RBrace
            | Token::Assign
            | Token::Arrow
            | Token::Pipe
            | Token::Fallback
            | Token::Eq
            | Token::Ne
            | Token::Le
            | Token::Ge
            | Token::And
            | Token::Or
            | Token::Then
            | Token::While
            | Token::Match
            | Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Percent => return false,
            _ => {}
        }
    }
    false
}

fn prev_non_comment_before(items: &[Item<'_>], idx: usize) -> Option<usize> {
    let mut cursor = idx;
    while cursor > 0 {
        cursor -= 1;
        if matches!(items[cursor], Item::Token { .. }) {
            return Some(cursor);
        }
    }
    None
}

fn item_raw<'a>(item: &'a Item<'a>) -> &'a str {
    match item {
        Item::Token { raw, .. }
        | Item::LineComment { raw, .. }
        | Item::BlockComment { raw, .. } => raw,
    }
}

fn would_merge_tokens(left: &str, right: &str) -> bool {
    let Some(l_last) = left.chars().next_back() else {
        return false;
    };
    let Some(r_first) = right.chars().next() else {
        return false;
    };
    let l_word = l_last.is_alphanumeric() || l_last == '_';
    let r_word = r_first.is_alphanumeric() || r_first == '_';
    if l_word && r_word {
        return true;
    }
    matches!(
        (l_last, r_first),
        ('|', '>')
            | ('|', '|')
            | ('=', '>')
            | ('=', '=')
            | ('-', '>')
            | ('!', '=')
            | ('<', '=')
            | ('>', '=')
            | ('&', '&')
            | ('.', '.')
            | (':', ':')
            | ('/', '/')
            | ('/', '*')
    )
}

fn scan_items(source: &str) -> Result<Vec<Item<'_>>, String> {
    let mut items = Vec::new();
    let mut remaining = source;

    loop {
        let mut newlines = 0usize;
        let mut ws_bytes = 0usize;
        for (idx, ch) in remaining.char_indices() {
            if !ch.is_whitespace() {
                break;
            }
            if ch == '\n' {
                newlines += 1;
            }
            ws_bytes = idx + ch.len_utf8();
        }
        remaining = &remaining[ws_bytes..];
        if remaining.is_empty() {
            break;
        }

        if remaining.starts_with("//") {
            let end = remaining.find('\n').unwrap_or(remaining.len());
            items.push(Item::LineComment {
                raw: &remaining[..end],
                newlines_before: newlines,
            });
            remaining = &remaining[end..];
            continue;
        }

        if remaining.starts_with("/*") {
            let Some(close) = remaining[2..].find("*/") else {
                return Err("Unterminated block comment".to_string());
            };
            let end = 2 + close + 2;
            items.push(Item::BlockComment {
                raw: &remaining[..end],
                newlines_before: newlines,
            });
            remaining = &remaining[end..];
            continue;
        }

        let (after, token) = lex_token(remaining).map_err(|_| {
            let preview: String = remaining.chars().take(32).collect();
            format!("Failed to lex token near `{preview}`")
        })?;
        let consumed = remaining.len() - after.len();
        if consumed == 0 {
            return Err("Formatter made zero progress while scanning tokens".to_string());
        }
        let attached_next = after.chars().next().is_some_and(|c| !c.is_whitespace());
        items.push(Item::Token {
            token,
            raw: &remaining[..consumed],
            newlines_before: newlines,
            attached_next,
        });
        remaining = after;
    }

    Ok(items)
}

fn verify_formatting_invariant(
    original: &str,
    formatted: &str,
    original_items: &[Item<'_>],
) -> Result<(), String> {
    let original_tokens = lex_tokens(original)?;
    let formatted_tokens = lex_tokens(formatted)?;
    if original_tokens != formatted_tokens {
        return Err(
            "Formatter invariant violated: formatted token stream differs from original"
                .to_string(),
        );
    }

    let formatted_items = scan_items(formatted)?;
    let original_comments: Vec<(bool, &str)> = original_items
        .iter()
        .filter_map(|item| match item {
            Item::LineComment { raw, .. } => Some((true, raw.trim_end())),
            Item::BlockComment { raw, .. } => Some((false, raw.trim_end())),
            Item::Token { .. } => None,
        })
        .collect();
    let formatted_comments: Vec<(bool, &str)> = formatted_items
        .iter()
        .filter_map(|item| match item {
            Item::LineComment { raw, .. } => Some((true, raw.trim_end())),
            Item::BlockComment { raw, .. } => Some((false, raw.trim_end())),
            Item::Token { .. } => None,
        })
        .collect();
    if original_comments != formatted_comments {
        return Err(
            "Formatter invariant violated: comments were altered during formatting".to_string(),
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_indentation_operators_pipes_and_preserves_comments() {
        let messy = r#"// Top comment
import project.math.{ add , double }

record Point{x:Int32,y:Int32}

fun compute: ( x: Int32 ,y: Int32 ) -> Result<Int32,String> = {
val sum=x+y;// inline note
val negated = -sum ;
sum > 0 then {
negated + 1 |> double
} else {
0
}
}
"#;
        let expected = r#"// Top comment
import project.math.{add, double}

record Point { x: Int32, y: Int32 }

fun compute: (x: Int32, y: Int32) -> Result<Int32, String> = {
    val sum = x + y; // inline note
    val negated = -sum;
    sum > 0 then {
        negated + 1 |> double
    } else {
        0
    }
}
"#;
        let first = format_source(messy).expect("messy source should format");
        assert_eq!(first, expected);
        let second = format_source(&first).expect("formatted source should re-format");
        assert_eq!(second, first, "formatter must be idempotent");
    }

    #[test]
    fn preserves_generic_angles_nested_generics_and_lambdas() {
        let input = r#"fun id: < T >( value: List< Option< T > > ) -> List< Option< T > > = {
    value map {|item|item}
}
"#;
        let expected = r#"fun id: <T>(value: List<Option<T>>) -> List<Option<T>> = {
    value map { |item| item }
}
"#;
        let formatted = format_source(input).unwrap();
        assert_eq!(formatted, expected);
        assert_eq!(format_source(&formatted).unwrap(), formatted);
    }

    #[test]
    fn preserves_string_escapes_numeric_literals_and_semicolons() {
        let input = "fun main: () -> () = {\n    val msg = \"a\\nb\\\"c\";\n    val num = 1_000;\n    msg |> println;\n    num |> print_int\n}\n";
        let formatted = format_source(input).unwrap();
        assert_eq!(formatted, input);
        assert_eq!(lex_tokens(input).unwrap(), lex_tokens(&formatted).unwrap());
    }

    #[test]
    fn collapses_multiple_blank_lines_and_strips_trailing_spaces() {
        let input = "\n\nfun a: () -> Int32 = { 1 }   \n\n\n\nfun b: () -> Int32 = { 2 }\t \n\n";
        let expected = "fun a: () -> Int32 = { 1 }\n\nfun b: () -> Int32 = { 2 }\n";
        assert_eq!(format_source(input).unwrap(), expected);
    }

    #[test]
    fn formats_pipe_scope_stages_idempotently() {
        let input = r#"fun total:(values:List<Int32>)->Int32={
values
|>map{it+1}
|>{
val seed=0
(it,seed)|>fold{|sum,value|sum+value}
}
}
"#;
        let expected = r#"fun total: (values: List<Int32>) -> Int32 = {
    values
    |> map { it + 1 }
    |> {
        val seed = 0
        (it, seed) |> fold { |sum, value| sum + value }
    }
}
"#;
        let formatted = format_source(input).unwrap();
        assert_eq!(formatted, expected);
        assert_eq!(format_source(&formatted).unwrap(), formatted);
    }

    #[test]
    fn formats_fallback_operator_idempotently() {
        let input = r#"fun port:(raw:String)->Int32={
val parsed=raw
|>string_trim
|>string_parse_int
??8080
|>clamp_port
val offset=raw string_parse_int??-1
val first=(1 Option::Some)??(2 Option::Some)??3
parsed+offset+first
}
"#;
        let expected = r#"fun port: (raw: String) -> Int32 = {
    val parsed = raw
    |> string_trim
    |> string_parse_int
    ?? 8080
    |> clamp_port
    val offset = raw string_parse_int ?? -1
    val first = (1 Option::Some) ?? (2 Option::Some) ?? 3
    parsed + offset + first
}
"#;
        let formatted = format_source(input).unwrap();
        assert_eq!(formatted, expected);
        assert_eq!(format_source(&formatted).unwrap(), formatted);
    }

    #[test]
    fn formats_semicolon_free_code_and_next_line_unary_minus_idempotently() {
        let input = r#"fun demo:(a:Int32)->Int32={
val x=1
-a
}

fun pipeline:(x:Int32)->Int32={
val y=x
y
|>double
|>increment
}
"#;
        let expected = r#"fun demo: (a: Int32) -> Int32 = {
    val x = 1
    -a
}

fun pipeline: (x: Int32) -> Int32 = {
    val y = x
    y
    |> double
    |> increment
}
"#;
        let first = format_source(input).unwrap();
        assert_eq!(first, expected);
        assert_eq!(format_source(&first).unwrap(), first);
    }
}
