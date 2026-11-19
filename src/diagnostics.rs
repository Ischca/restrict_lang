//! User-facing diagnostic formatting helpers.

use crate::release_surface::ReleaseSurfaceError;
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
    match error {
        TypeError::NonCallableVerb { name, .. } => {
            if let Some((line, column, source_line)) = find_osv_verb(source, name) {
                message.push_str(&format!(
                    "\n  at line {line}, column {column}\n  {source_line}\n  {}^ non-callable OSV verb",
                    " ".repeat(column.saturating_sub(1))
                ));
            }
        }
        TypeError::AffineViolation(name) => {
            let mask = code_byte_mask(source);
            if let Some((first_offset, second_offset)) = locate_affine_uses(source, &mask, name) {
                let (line2, col2) = line_column(source, second_offset);
                message.push_str(&format!("\n  at line {line2}, column {col2} (second use)"));
                if let Some((source_line, caret_column)) = source_line_at(source, second_offset) {
                    message.push_str(&format!(
                        "\n  {source_line}\n  {}^ used again here",
                        " ".repeat(caret_column.saturating_sub(1))
                    ));
                }
                let (line1, col1) = line_column(source, first_offset);
                message.push_str(&format!("\n  first used at line {line1}, column {col1}"));
                if let Some((source_line, caret_column)) = source_line_at(source, first_offset) {
                    message.push_str(&format!(
                        "\n  {source_line}\n  {}^ first consumed here",
                        " ".repeat(caret_column.saturating_sub(1))
                    ));
                }
            }
        }
        TypeError::UndefinedVariable(name) => {
            let mask = code_byte_mask(source);
            if let Some(offset) = find_value_ident_usage(source, &mask, name) {
                append_location_snippet(&mut message, source, offset, "undefined variable");
            }
        }
        TypeError::UndefinedFunction(name) => {
            let mask = code_byte_mask(source);
            if let Some(offset) = find_function_call_usage(source, &mask, name) {
                append_location_snippet(&mut message, source, offset, "undefined function");
            }
        }
        TypeError::UndefinedRecord(name) => {
            let mask = code_byte_mask(source);
            if let Some(offset) = find_record_lit_or_ident(source, &mask, name) {
                append_location_snippet(&mut message, source, offset, "unknown record type");
            }
        }
        TypeError::UnknownField { record, field } => {
            let mask = code_byte_mask(source);
            if let Some(offset) = find_field_usage(source, &mask, field) {
                let label = format!("unknown field '{field}' on '{record}'");
                append_location_snippet(&mut message, source, offset, &label);
            }
        }
        TypeError::ImmutableReassignment(name) => {
            let mask = code_byte_mask(source);
            if let Some(offset) = find_reassignment_target(source, &mask, name) {
                append_location_snippet(
                    &mut message,
                    source,
                    offset,
                    "cannot reassign immutable binding",
                );
            }
        }
        TypeError::UnknownType(name) => {
            let mask = code_byte_mask(source);
            if let Some(offset) = find_type_name_usage(source, &mask, name) {
                append_location_snippet(&mut message, source, offset, "unknown type");
            }
        }
        TypeError::UndefinedMethod { method, .. } => {
            let mask = code_byte_mask(source);
            if let Some(offset) = find_function_call_usage(source, &mask, method) {
                append_location_snippet(&mut message, source, offset, "undefined method");
            }
        }
        _ => {}
    }
    message
}

pub fn format_release_surface_error(source: &str, error: &ReleaseSurfaceError) -> String {
    let text = error.to_string();
    let mut message = format!("Release surface error: {text}");
    let mask = code_byte_mask(source);
    if let Some(offset) = locate_release_surface_offset(source, &mask, &text) {
        append_location_snippet(
            &mut message,
            source,
            offset,
            "exported item violates host ABI surface",
        );
    }
    message
}

fn append_location_snippet(message: &mut String, source: &str, offset: usize, label: &str) {
    let (line, column) = line_column(source, offset);
    message.push_str(&format!("\n  at line {line}, column {column}"));
    if let Some((source_line, caret_column)) = source_line_at(source, offset) {
        message.push_str(&format!(
            "\n  {source_line}\n  {}^ {label}",
            " ".repeat(caret_column.saturating_sub(1))
        ));
    }
}

fn locate_release_surface_offset(source: &str, mask: &[bool], message: &str) -> Option<usize> {
    if let Some(name) = quoted_after(message, "Exported function '")
        .or_else(|| quoted_after(message, "Exported generic function '"))
        .or_else(|| quoted_after(message, "Function '"))
    {
        if let Some(offset) = find_decl_name(source, mask, "fun", name) {
            return Some(offset);
        }
    }
    if let Some(name) = quoted_after(message, "Exported top-level binding '") {
        if let Some(offset) = find_binding_decl_name(source, mask, name) {
            return Some(offset);
        }
    }
    if let Some(name) = quoted_after(message, "Record '") {
        if let Some(offset) = find_decl_name(source, mask, "record", name) {
            return Some(offset);
        }
    }
    if let Some(name) = quoted_after(message, "Enum '") {
        if let Some(offset) = find_decl_name(source, mask, "enum", name) {
            return Some(offset);
        }
    }
    if message.contains("with lifetime") {
        let mut cursor = 0;
        while let Some(pos) = find_word(source, mask, "with", cursor) {
            let next = skip_non_code_ws(source, mask, pos + 4);
            if starts_with_word_at(source, mask, next, "lifetime") {
                return Some(pos);
            }
            cursor = pos + 4;
        }
    }
    if message.contains("Exported top-level bindings must be immutable") {
        for kw in ["pub", "export"] {
            let mut cursor = 0;
            while let Some(pos) = find_word(source, mask, kw, cursor) {
                let next = skip_non_code_ws(source, mask, pos + kw.len());
                if starts_with_word_at(source, mask, next, "mut") {
                    return Some(next);
                }
                cursor = pos + kw.len();
            }
        }
    }
    None
}

fn quoted_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    let end = text[start..].find('\'')? + start;
    Some(&text[start..end])
}

fn find_decl_name(source: &str, mask: &[bool], keyword: &str, name: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, keyword, cursor) {
        let next = skip_non_code_ws(source, mask, pos + keyword.len());
        if starts_with_word_at(source, mask, next, name) {
            return Some(next);
        }
        cursor = pos + keyword.len();
    }
    None
}

fn find_binding_decl_name(source: &str, mask: &[bool], name: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, "val", cursor) {
        let next = skip_non_code_ws(source, mask, pos + 3);
        if starts_with_word_at(source, mask, next, name) {
            return Some(next);
        }
        cursor = pos + 3;
    }
    None
}

fn locate_affine_uses(source: &str, mask: &[bool], var_name: &str) -> Option<(usize, usize)> {
    let scopes = collect_function_bodies(source, mask);
    for (fn_start, body_start, body_end) in &scopes {
        let declared_in_params =
            find_word_in_range(source, mask, var_name, *fn_start, *body_start).is_some();
        let uses = collect_var_uses_in_range(
            source,
            mask,
            var_name,
            *body_start,
            *body_end,
            declared_in_params,
        );
        if uses.len() >= 2 {
            return Some((uses[0], uses[1]));
        }
    }

    let global_uses = collect_var_uses_in_range(source, mask, var_name, 0, source.len(), true);
    if global_uses.len() >= 2 {
        Some((global_uses[0], global_uses[1]))
    } else {
        None
    }
}

fn collect_function_bodies(source: &str, mask: &[bool]) -> Vec<(usize, usize, usize)> {
    let mut bodies = Vec::new();
    let mut cursor = 0;
    while let Some(fun_pos) = find_word(source, mask, "fun", cursor) {
        let mut idx = fun_pos + 3;
        let mut found_assign = false;
        while idx < source.len() {
            if mask[idx]
                && source.as_bytes()[idx] == b'='
                && !source[idx..].starts_with("==")
                && !source[idx..].starts_with("=>")
            {
                found_assign = true;
                idx += 1;
                break;
            }
            if mask[idx] && source.as_bytes()[idx] == b'{' {
                break;
            }
            idx += 1;
        }
        if !found_assign {
            cursor = fun_pos + 3;
            continue;
        }
        let body_open = skip_non_code_ws(source, mask, idx);
        if source.as_bytes().get(body_open) == Some(&b'{') {
            let body_end = skip_balanced(source, mask, body_open, '{', '}');
            bodies.push((fun_pos, body_open, body_end));
            cursor = body_end;
        } else {
            cursor = fun_pos + 3;
        }
    }
    bodies
}

fn collect_var_uses_in_range(
    source: &str,
    mask: &[bool],
    var_name: &str,
    start: usize,
    end: usize,
    already_in_scope: bool,
) -> Vec<usize> {
    let mut uses = Vec::new();
    let mut in_scope = already_in_scope;
    let mut cursor = start;

    while let Some(pos) = find_word_in_range(source, mask, var_name, cursor, end) {
        let after = pos + var_name.len();
        if is_member_or_namespace_access(source, mask, pos)
            || is_record_field_label(source, mask, pos, after)
            || is_binding_or_pattern_site(source, mask, pos, after)
            || is_reassignment_lhs(source, mask, pos, after)
        {
            if is_binding_or_pattern_site(source, mask, pos, after) {
                in_scope = true;
            }
            cursor = after;
            continue;
        }

        if in_scope {
            uses.push(pos);
        }
        cursor = after;
    }
    uses
}

fn find_word_in_range(
    source: &str,
    mask: &[bool],
    word: &str,
    start: usize,
    end: usize,
) -> Option<usize> {
    let slice_end = end.min(source.len());
    let mut search_from = start;
    while search_from < slice_end {
        let rel = source[search_from..slice_end].find(word)?;
        let pos = search_from + rel;
        if starts_with_word_at(source, mask, pos, word) {
            return Some(pos);
        }
        search_from = pos + word.len();
    }
    None
}

fn is_member_or_namespace_access(source: &str, mask: &[bool], pos: usize) -> bool {
    matches!(prev_non_code_byte(source, mask, pos), Some(b'.'))
        || prev_non_code_two_bytes(source, mask, pos) == Some(*b"::")
}

fn is_record_field_label(source: &str, mask: &[bool], pos: usize, after: usize) -> bool {
    let next = skip_non_code_ws(source, mask, after);
    if !source[next..].starts_with(':') || source[next..].starts_with("::") {
        return false;
    }
    matches!(prev_non_code_byte(source, mask, pos), Some(b'{' | b','))
}

fn is_binding_or_pattern_site(source: &str, mask: &[bool], pos: usize, after: usize) -> bool {
    let Some(prev_word_start) = prev_ident_start(source, mask, pos) else {
        return is_lambda_param_binder(source, mask, pos, after)
            || is_match_arm_binder(source, mask, after);
    };
    let prev_word = &source
        [prev_word_start..scan_ident_end(source, mask, prev_word_start).unwrap_or(prev_word_start)];
    if matches!(
        prev_word,
        "val" | "mut" | "fun" | "record" | "enum" | "form" | "takes" | "import"
    ) {
        return true;
    }
    let prev_byte = prev_non_code_byte(source, mask, pos);
    if matches!(prev_byte, Some(b'(' | b',' | b'|')) {
        let next = skip_non_code_ws(source, mask, after);
        if source[next..].starts_with(':') && !source[next..].starts_with("::") {
            return true;
        }
    }
    is_lambda_param_binder(source, mask, pos, after) || is_match_arm_binder(source, mask, after)
}

fn is_lambda_param_binder(source: &str, mask: &[bool], pos: usize, after: usize) -> bool {
    let prev = prev_non_code_byte(source, mask, pos);
    let next = skip_non_code_ws(source, mask, after);
    matches!(prev, Some(b'|'))
        && (source[next..].starts_with('|')
            || source[next..].starts_with(':')
            || source[next..].starts_with(','))
}

fn is_match_arm_binder(source: &str, mask: &[bool], after: usize) -> bool {
    let next = skip_non_code_ws(source, mask, after);
    if source[next..].starts_with("=>") {
        return true;
    }
    if source[next..].starts_with(')') {
        let after_paren = skip_non_code_ws(source, mask, next + 1);
        return source[after_paren..].starts_with("=>");
    }
    false
}

fn is_reassignment_lhs(source: &str, mask: &[bool], pos: usize, after: usize) -> bool {
    let next = skip_non_code_ws(source, mask, after);
    if !source[next..].starts_with('=')
        || source[next..].starts_with("==")
        || source[next..].starts_with("=>")
    {
        return false;
    }
    !matches!(prev_ident_word(source, mask, pos), Some("val" | "mut"))
}

fn prev_ident_start(source: &str, mask: &[bool], before: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut idx = before;
    while idx > 0 {
        idx -= 1;
        if !mask[idx] || bytes[idx].is_ascii_whitespace() {
            continue;
        }
        if !is_identifier_continue(bytes[idx] as char) {
            return None;
        }
        while idx > 0 && mask[idx - 1] && is_identifier_continue(bytes[idx - 1] as char) {
            idx -= 1;
        }
        return Some(idx);
    }
    None
}

fn prev_ident_word<'a>(source: &'a str, mask: &[bool], before: usize) -> Option<&'a str> {
    let start = prev_ident_start(source, mask, before)?;
    let end = scan_ident_end(source, mask, start)?;
    Some(&source[start..end])
}

fn find_value_ident_usage(source: &str, mask: &[bool], name: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, name, cursor) {
        let after = pos + name.len();
        if !is_member_or_namespace_access(source, mask, pos)
            && !is_record_field_label(source, mask, pos, after)
            && !is_binding_or_pattern_site(source, mask, pos, after)
        {
            return Some(pos);
        }
        cursor = after;
    }
    find_word(source, mask, name, 0)
}

fn find_function_call_usage(source: &str, mask: &[bool], name: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, name, cursor) {
        let after = pos + name.len();
        let is_decl = matches!(prev_ident_word(source, mask, pos), Some("fun" | "import"));
        if !is_decl && !is_member_or_namespace_access(source, mask, pos) {
            return Some(pos);
        }
        cursor = after;
    }
    None
}

fn find_record_lit_or_ident(source: &str, mask: &[bool], name: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, name, cursor) {
        let after = pos + name.len();
        let next = skip_non_code_ws(source, mask, after);
        if source[next..].starts_with('{') {
            return Some(pos);
        }
        cursor = after;
    }
    find_value_ident_usage(source, mask, name)
}

fn find_field_usage(source: &str, mask: &[bool], field: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, field, cursor) {
        if matches!(prev_non_code_byte(source, mask, pos), Some(b'.')) {
            return Some(pos);
        }
        cursor = pos + field.len();
    }
    cursor = 0;
    while let Some(pos) = find_word(source, mask, field, cursor) {
        let after = pos + field.len();
        if is_record_field_label(source, mask, pos, after) {
            return Some(pos);
        }
        cursor = after;
    }
    None
}

fn find_reassignment_target(source: &str, mask: &[bool], name: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, name, cursor) {
        let after = pos + name.len();
        if is_reassignment_lhs(source, mask, pos, after) {
            return Some(pos);
        }
        cursor = after;
    }
    None
}

fn find_type_name_usage(source: &str, mask: &[bool], name: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, name, cursor) {
        let prev = prev_non_code_byte(source, mask, pos);
        let prev_kw = prev_ident_word(source, mask, pos);
        if matches!(prev, Some(b':' | b'<' | b',' | b'>'))
            || matches!(prev_kw, Some("as"))
            || source[..pos].trim_end().ends_with("->")
        {
            return Some(pos);
        }
        cursor = pos + name.len();
    }
    find_word(source, mask, name, 0)
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
                let detail = error.input.trim();
                if let Some(offset) = find_custom_parser_failure_offset(source, detail) {
                    let (line, column) = line_column(source, offset);
                    let mut message =
                        format!("{phase} error at line {line}, column {column}: {detail}");
                    if let Some((source_line, caret_column)) = source_line_at(source, offset) {
                        message.push_str(&format!(
                            "\n  {source_line}\n  {}^ parsing stopped here",
                            " ".repeat(caret_column.saturating_sub(1))
                        ));
                    }
                    return message;
                }
                return format!("{phase} error: {detail}");
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

fn find_custom_parser_failure_offset(source: &str, detail: &str) -> Option<usize> {
    let mask = code_byte_mask(source);
    if detail.contains("stale syntax `let`") {
        return find_word(source, &mask, "let", 0);
    }
    if detail.contains("stale syntax `if (...)`") {
        let mut cursor = 0;
        while let Some(pos) = find_word(source, &mask, "if", cursor) {
            let next = skip_non_code_ws(source, &mask, pos + 2);
            if source[next..].starts_with('(') {
                return Some(pos);
            }
            cursor = pos + 2;
        }
        return None;
    }
    if detail.contains("stale syntax `val mut`") {
        let mut cursor = 0;
        while let Some(pos) = find_word(source, &mask, "val", cursor) {
            let next = skip_non_code_ws(source, &mask, pos + 3);
            if starts_with_word_at(source, &mask, next, "mut") {
                return Some(pos);
            }
            cursor = pos + 3;
        }
        return None;
    }
    if detail.contains("stale syntax `Unit`") {
        return find_word(source, &mask, "Unit", 0);
    }
    if detail.contains("stale function declaration syntax") {
        let mut cursor = 0;
        while let Some(pos) = find_word(source, &mask, "fun", cursor) {
            let after_fun = skip_non_code_ws(source, &mask, pos + 3);
            if let Some(after_name) = scan_ident_end(source, &mask, after_fun) {
                let next = skip_non_code_ws(source, &mask, after_name);
                if source[next..].starts_with('=') {
                    return Some(pos);
                }
                if source[next..].starts_with(':') {
                    let mut after_colon = skip_non_code_ws(source, &mask, next + 1);
                    if source[after_colon..].starts_with('<') {
                        after_colon = skip_balanced(source, &mask, after_colon, '<', '>');
                        after_colon = skip_non_code_ws(source, &mask, after_colon);
                    }
                    if !source[after_colon..].starts_with('(') {
                        return Some(pos);
                    }
                }
            }
            cursor = pos + 3;
        }
        return None;
    }
    if detail.contains("generic forms are not supported yet") {
        let mut cursor = 0;
        while let Some(pos) = find_word(source, &mask, "form", cursor) {
            let after_form = skip_non_code_ws(source, &mask, pos + 4);
            if let Some(after_name) = scan_ident_end(source, &mask, after_form) {
                let next = skip_non_code_ws(source, &mask, after_name);
                if source[next..].starts_with('<') {
                    return Some(pos);
                }
            }
            cursor = pos + 4;
        }
        return find_inside_form_or_takes(source, &mask, "form", |s, m, body| {
            let mut c = body;
            while let Some(fpos) = find_word(s, m, "fun", c) {
                let a = skip_non_code_ws(s, m, fpos + 3);
                if let Some(e) = scan_ident_end(s, m, a) {
                    let colon = skip_non_code_ws(s, m, e);
                    if s[colon..].starts_with(':') {
                        let lt = skip_non_code_ws(s, m, colon + 1);
                        if s[lt..].starts_with('<') {
                            return Some(fpos);
                        }
                    }
                }
                c = fpos + 3;
            }
            None
        });
    }
    if detail.contains("generic or temporal takes targets") {
        let mut cursor = 0;
        while let Some(pos) = find_word(source, &mask, "takes", cursor) {
            let before = prev_non_code_byte(source, &mask, pos);
            if matches!(before, Some(b'>')) || before_has_tilde_prefix(source, &mask, pos) {
                return Some(pos);
            }
            let after_takes = skip_non_code_ws(source, &mask, pos + 5);
            if let Some(after_form) = scan_ident_end(source, &mask, after_takes) {
                let next = skip_non_code_ws(source, &mask, after_form);
                if source[next..].starts_with('<') || source[next..].starts_with('~') {
                    return Some(pos);
                }
            }
            cursor = pos + 5;
        }
        return find_word(source, &mask, "takes", 0);
    }
    if detail.contains("conditional takes declarations") {
        return find_word(source, &mask, "takes", 0).and_then(|pos| {
            let after_takes = skip_non_code_ws(source, &mask, pos + 5);
            let after_form = scan_ident_end(source, &mask, after_takes)?;
            let next = skip_non_code_ws(source, &mask, after_form);
            if starts_with_word_at(source, &mask, next, "where") {
                Some(next)
            } else {
                Some(pos)
            }
        });
    }
    if detail.contains("takes declarations cannot be public") {
        for kw in ["pub", "export"] {
            let mut cursor = 0;
            while let Some(pos) = find_word(source, &mask, kw, cursor) {
                let next = skip_non_code_ws(source, &mask, pos + kw.len());
                if let Some(after_target) = scan_ident_end(source, &mask, next) {
                    let maybe_takes = skip_non_code_ws(source, &mask, after_target);
                    if starts_with_word_at(source, &mask, maybe_takes, "takes") {
                        return Some(pos);
                    }
                }
                cursor = pos + kw.len();
            }
        }
        return None;
    }
    if detail.contains("source-level associated form types") {
        return find_inside_form_or_takes(source, &mask, "form", |s, m, body| {
            find_word(s, m, "type", body)
        })
        .or_else(|| {
            find_inside_form_or_takes(source, &mask, "takes", |s, m, body| {
                find_word(s, m, "type", body)
            })
        });
    }
    if detail.contains("default form method bodies") {
        return find_inside_form_or_takes(source, &mask, "form", |s, m, body| {
            let mut i = body;
            while i < s.len() {
                if m[i] && s.as_bytes()[i] == b'}' {
                    break;
                }
                if m[i]
                    && s.as_bytes()[i] == b'='
                    && !s[i..].starts_with("==")
                    && !s[i..].starts_with("=>")
                {
                    return Some(i);
                }
                i += 1;
            }
            None
        });
    }
    if detail.contains("re-exports are unsupported") {
        for kw in ["pub", "export"] {
            let mut cursor = 0;
            while let Some(pos) = find_word(source, &mask, kw, cursor) {
                let next = skip_non_code_ws(source, &mask, pos + kw.len());
                if starts_with_word_at(source, &mask, next, "import") {
                    return Some(pos);
                }
                cursor = pos + kw.len();
            }
        }
        return None;
    }
    if detail.contains("string import paths and import aliases") {
        let mut cursor = 0;
        while let Some(pos) = find_word(source, &mask, "import", cursor) {
            let next = skip_non_code_ws(source, &mask, pos + 6);
            if source.as_bytes().get(next) == Some(&b'"') {
                return Some(next);
            }
            if let Some(as_pos) = find_word(source, &mask, "as", next) {
                let line_end = source[pos..].find('\n').map_or(source.len(), |n| pos + n);
                if as_pos < line_end {
                    return Some(as_pos);
                }
            }
            cursor = pos + 6;
        }
        return None;
    }
    if detail
        .contains("built-in constructors for Option or Result do not take source type arguments")
    {
        for prefix in ["Option::Some", "Option::None", "Result::Ok", "Result::Err"] {
            let mut cursor = 0;
            while let Some(pos) = find_code_substring(source, &mask, prefix, cursor) {
                let after = pos + prefix.len();
                let after_ok = source
                    .get(after..)
                    .and_then(|s| s.chars().next())
                    .is_none_or(|ch| !is_identifier_continue(ch));
                if after_ok {
                    let next = skip_non_code_ws(source, &mask, after);
                    if source[next..].starts_with('<') {
                        return Some(pos);
                    }
                }
                cursor = after;
            }
        }
        return None;
    }
    if detail.contains("unqualified Option value construction") {
        for word in ["Some", "None"] {
            let mut cursor = 0;
            while let Some(pos) = find_word(source, &mask, word, cursor) {
                if prev_non_code_two_bytes(source, &mask, pos) != Some(*b"::")
                    && !is_match_arm_pattern(source, &mask, pos + word.len())
                {
                    return Some(pos);
                }
                cursor = pos + word.len();
            }
        }
        return None;
    }
    if detail.contains("traditional Result value construction") {
        for word in ["Ok", "Err"] {
            let mut cursor = 0;
            while let Some(pos) = find_word(source, &mask, word, cursor) {
                if prev_non_code_two_bytes(source, &mask, pos) != Some(*b"::") {
                    let next = skip_non_code_ws(source, &mask, pos + word.len());
                    if source[next..].starts_with('(')
                        && !is_match_arm_pattern(source, &mask, pos + word.len())
                    {
                        return Some(pos);
                    }
                }
                cursor = pos + word.len();
            }
        }
        return None;
    }
    if detail.contains("traditional calls like") {
        return find_traditional_call_offset(source, &mask);
    }
    if detail.contains("anonymous exact-once scopes") {
        let mut i = 0;
        while i < source.len() {
            if mask[i] && source.as_bytes()[i] == b'{' {
                let next = skip_non_code_ws(source, &mask, i + 1);
                if source[next..].starts_with("||") {
                    return Some(next);
                }
                if source[next..].starts_with('|') {
                    let end = skip_balanced(source, &mask, next, '|', '|');
                    if source[next..end].contains(',') {
                        return Some(next);
                    }
                }
            }
            i += 1;
        }
        return None;
    }
    None
}

fn find_traditional_call_offset(source: &str, mask: &[bool]) -> Option<usize> {
    const NON_CALL_KEYWORDS: &[&str] = &[
        "fun", "val", "mut", "record", "enum", "form", "takes", "impl", "context", "match", "then",
        "else", "while", "if", "with", "within", "where", "import", "export", "pub", "as", "clone",
        "freeze", "Ok", "Err", "Some", "None", "true", "false",
    ];

    let mut i = 0;
    while i < source.len() {
        if !mask[i] {
            i += 1;
            continue;
        }
        let ch = source[i..].chars().next()?;
        if !(ch.is_ascii_alphabetic() || ch == '_') {
            i += ch.len_utf8();
            continue;
        }
        let Some(end) = scan_ident_end(source, mask, i) else {
            i += ch.len_utf8();
            continue;
        };
        let ident = &source[i..end];
        if !NON_CALL_KEYWORDS.contains(&ident) {
            let next = skip_non_code_ws(source, mask, end);
            if source[next..].starts_with('(') && !is_match_arm_pattern(source, mask, end) {
                return Some(i);
            }
        }
        i = end;
    }
    None
}

fn is_match_arm_pattern(source: &str, mask: &[bool], after_ident: usize) -> bool {
    let mut next = skip_non_code_ws(source, mask, after_ident);
    if source[next..].starts_with('(') {
        next = skip_balanced(source, mask, next, '(', ')');
        next = skip_non_code_ws(source, mask, next);
    }
    source[next..].starts_with("=>")
}

fn find_inside_form_or_takes<F>(source: &str, mask: &[bool], keyword: &str, f: F) -> Option<usize>
where
    F: Fn(&str, &[bool], usize) -> Option<usize>,
{
    let mut cursor = 0;
    while let Some(pos) = find_word(source, mask, keyword, cursor) {
        let mut i = pos + keyword.len();
        while i < source.len() {
            if mask[i] && source.as_bytes()[i] == b'{' {
                if let Some(found) = f(source, mask, i + 1) {
                    return Some(found);
                }
                i = skip_balanced(source, mask, i, '{', '}');
                break;
            }
            i += 1;
        }
        cursor = i;
    }
    None
}

fn code_byte_mask(source: &str) -> Vec<bool> {
    let bytes = source.as_bytes();
    let mut mask = vec![true; bytes.len()];
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                mask[i] = false;
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            mask[i] = false;
            mask[i + 1] = false;
            i += 2;
            while i < bytes.len() {
                mask[i] = false;
                if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    mask[i + 1] = false;
                    i += 2;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'"' {
            mask[i] = false;
            i += 1;
            while i < bytes.len() {
                mask[i] = false;
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    mask[i + 1] = false;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'\'' {
            mask[i] = false;
            i += 1;
            while i < bytes.len() {
                mask[i] = false;
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    mask[i + 1] = false;
                    i += 2;
                    continue;
                }
                if bytes[i] == b'\'' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    mask
}

fn skip_non_code_ws(source: &str, mask: &[bool], mut idx: usize) -> usize {
    let bytes = source.as_bytes();
    while idx < bytes.len() {
        if !mask[idx] || bytes[idx].is_ascii_whitespace() {
            idx += 1;
        } else {
            break;
        }
    }
    idx
}

fn skip_balanced(source: &str, mask: &[bool], start: usize, open: char, close: char) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    for (rel, ch) in source[start..].char_indices() {
        let abs = start + rel;
        if !mask[abs] {
            continue;
        }
        if ch == open && (open != close || depth == 0) {
            depth += 1;
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return abs + ch.len_utf8();
            }
        }
        i = abs + ch.len_utf8();
    }
    i
}

fn scan_ident_end(source: &str, mask: &[bool], start: usize) -> Option<usize> {
    if start >= source.len() || !mask[start] {
        return None;
    }
    let first = source[start..].chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let tail_start = start + first.len_utf8();
    let mut end = tail_start;
    for (rel, ch) in source[tail_start..].char_indices() {
        let abs = tail_start + rel;
        if !mask[abs] || !is_identifier_continue(ch) {
            return Some(abs);
        }
        end = abs + ch.len_utf8();
    }
    Some(end)
}

fn starts_with_word_at(source: &str, mask: &[bool], start: usize, word: &str) -> bool {
    if !source[start..].starts_with(word) {
        return false;
    }
    let end = start + word.len();
    if !(start..end).all(|idx| mask[idx]) {
        return false;
    }
    let before_ok = source[..start]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_identifier_continue(ch));
    let after_ok = source[end..]
        .chars()
        .next()
        .is_none_or(|ch| !is_identifier_continue(ch));
    before_ok && after_ok
}

fn find_word(source: &str, mask: &[bool], word: &str, from: usize) -> Option<usize> {
    let mut search_from = from;
    while let Some(rel) = source[search_from..].find(word) {
        let pos = search_from + rel;
        if starts_with_word_at(source, mask, pos, word) {
            return Some(pos);
        }
        search_from = pos + word.len();
    }
    None
}

fn find_code_substring(source: &str, mask: &[bool], needle: &str, from: usize) -> Option<usize> {
    let mut search_from = from;
    while let Some(rel) = source[search_from..].find(needle) {
        let pos = search_from + rel;
        if (pos..pos + needle.len()).all(|idx| mask[idx]) {
            return Some(pos);
        }
        search_from = pos + needle.len();
    }
    None
}

fn prev_non_code_byte(source: &str, mask: &[bool], before: usize) -> Option<u8> {
    let bytes = source.as_bytes();
    let mut idx = before;
    while idx > 0 {
        idx -= 1;
        if mask[idx] && !bytes[idx].is_ascii_whitespace() {
            return Some(bytes[idx]);
        }
    }
    None
}

fn prev_non_code_two_bytes(source: &str, mask: &[bool], before: usize) -> Option<[u8; 2]> {
    let bytes = source.as_bytes();
    let mut idx = before;
    while idx > 0 {
        idx -= 1;
        if mask[idx] && !bytes[idx].is_ascii_whitespace() {
            if idx > 0 && mask[idx - 1] {
                return Some([bytes[idx - 1], bytes[idx]]);
            }
            return None;
        }
    }
    None
}

fn before_has_tilde_prefix(source: &str, mask: &[bool], before: usize) -> bool {
    let mut idx = before;
    let bytes = source.as_bytes();
    while idx > 0 {
        idx -= 1;
        if !mask[idx] || bytes[idx].is_ascii_whitespace() {
            continue;
        }
        while idx > 0 && mask[idx] && is_identifier_continue(bytes[idx] as char) {
            idx -= 1;
        }
        return mask[idx] && bytes[idx] == b'~';
    }
    false
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
        assert!(message.contains("at line 1, column 1"), "{message}");
        assert!(message.contains("form Printable<T> {"), "{message}");
    }

    #[test]
    fn custom_parser_failures_are_untruncated_and_report_line_and_column() {
        let source = r#"fun parse_ok: (flag: Boolean) -> Result<Int32, String> = {
    val existing = 1 Result::Ok;
    existing match {
        Ok(value) => { value Result::Ok }
        Err(msg) => { msg Result::Err }
    };
    Ok(42)
}
"#;
        let err = parse_program(source).expect_err("traditional Ok(...) should fail");
        let message = format_parse_error(source, err);

        assert!(
            message.contains("write `value Result::Ok` or `error Result::Err` (pipe is optional)"),
            "custom error should not be truncated at 120 chars: {message}"
        );
        assert!(message.contains("at line 7, column 5"), "{message}");
        assert!(message.contains("    Ok(42)"), "{message}");
        assert!(message.contains("    ^ parsing stopped here"), "{message}");
    }

    #[test]
    fn builtin_constructor_type_arg_error_is_untruncated_and_located() {
        let source = "fun main: () -> Option<Int32> = {\n    () Option::None<Int32>\n}\n";
        let err = parse_program(source).expect_err("type args on Option::None should fail");
        let message = format_parse_error(source, err);

        assert!(
            message.contains("so provide the full type through an annotation or typed context"),
            "full >120-char message must be preserved: {message}"
        );
        assert!(message.contains("at line 2, column 8"), "{message}");
        assert!(message.contains("Option::None<Int32>"), "{message}");
    }

    #[test]
    fn affine_violation_locates_both_first_and_second_use_skipping_comments_and_strings() {
        let source = r#"fun use_twice: () -> String = {
    // val raw = "commented raw"
    val label = "raw";
    val raw = "payload";
    val first = raw;
    raw
}
"#;
        let message = format_type_error(source, &TypeError::AffineViolation("raw".to_string()));
        assert!(
            message.contains("at line 6, column 5 (second use)"),
            "{message}"
        );
        assert!(
            message.contains("first used at line 5, column 17"),
            "{message}"
        );
        assert!(message.contains("^ first consumed here"), "{message}");
        assert!(message.contains("^ used again here"), "{message}");
    }

    #[test]
    fn undefined_function_is_located_without_semicolon_hint() {
        let source = r#"fun main: () -> () = {
    val msg = "ready"
    msg println_typo
}
"#;
        let message = format_type_error(
            source,
            &TypeError::UndefinedFunction("println_typo".to_string()),
        );
        assert!(message.contains("at line 3, column 9"), "{message}");
        assert!(
            !message.contains("does not end with ';'"),
            "semicolon-free multi-line val bindings should not emit stale semicolon hints: {message}"
        );
    }

    #[test]
    fn undefined_variable_record_field_and_immutable_reassignment_are_located() {
        let source = r#"fun demo: () -> Int32 = {
    val count = 1;
    count = 2;
    val p = MissingRec { x: 1 };
    p.bad_field + missing_ident
}
"#;
        let reassign = format_type_error(
            source,
            &TypeError::ImmutableReassignment("count".to_string()),
        );
        assert!(reassign.contains("at line 3, column 5"), "{reassign}");

        let rec = format_type_error(
            source,
            &TypeError::UndefinedRecord("MissingRec".to_string()),
        );
        assert!(rec.contains("at line 4, column 13"), "{rec}");

        let field = format_type_error(
            source,
            &TypeError::UnknownField {
                record: "Point".to_string(),
                field: "bad_field".to_string(),
            },
        );
        assert!(field.contains("at line 5, column 7"), "{field}");

        let var = format_type_error(
            source,
            &TypeError::UndefinedVariable("missing_ident".to_string()),
        );
        assert!(var.contains("at line 5, column 19"), "{var}");
    }

    #[test]
    fn release_surface_error_diagnostic_locates_exported_function() {
        let source = "pub fun bad_export: () -> String = {\n    \"ok\"\n}\n";
        let (_, program) = parse_program(source).unwrap();
        let mut checker = crate::TypeChecker::new();
        checker.check_program(&program).unwrap();
        let err =
            crate::check_release_surface(&program, &checker, crate::HostAbiProfile::V001Scalar)
                .unwrap_err();
        let formatted = format_release_surface_error(source, &err);

        assert!(formatted.contains("Release surface error: Exported function 'bad_export'"));
        assert!(formatted.contains("at line 1, column 9"), "{formatted}");
        assert!(
            formatted.contains("^ exported item violates host ABI surface"),
            "{formatted}"
        );
    }
}
