//! Deterministic release optimization for compiler-generated WebAssembly text.
//!
//! The production code generator still lowers function bodies directly from
//! the checked AST. Until Wasm MIR owns that lowering boundary, this pass keeps
//! release optimization deliberately narrow: it removes unreachable functions
//! and the named types and globals made unused by that removal. It never
//! rewrites instructions or changes source-level semantics.

use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReleaseOptimizationReport {
    pub removed_functions: usize,
    pub removed_function_imports: usize,
    pub removed_types: usize,
    pub removed_globals: usize,
    pub removed_tables: usize,
    pub removed_elements: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormKind {
    Function,
    FunctionImport,
    Type,
    Global,
    Table,
    Element,
    Export,
    Start,
    Other,
}

#[derive(Debug)]
struct TopLevelForm<'a> {
    start: usize,
    end: usize,
    tokens: Vec<&'a str>,
    kind: FormKind,
    name: Option<&'a str>,
}

pub fn eliminate_unreachable(wat: &str) -> Result<(String, ReleaseOptimizationReport), String> {
    let forms = top_level_forms(wat)?;
    let mut functions = HashMap::new();
    let mut function_imports = HashMap::new();
    let mut elements = Vec::new();
    let mut roots = VecDeque::new();
    let mut exported_table = false;

    for (index, form) in forms.iter().enumerate() {
        match form.kind {
            FormKind::Function => {
                if let Some(name) = form.name {
                    if functions.insert(name, index).is_some() {
                        return Err(format!("duplicate generated function name '{name}'"));
                    }
                    if contains_inline_export(&form.tokens) {
                        roots.push_back(name);
                    }
                }
            }
            FormKind::FunctionImport => {
                if let Some(name) = form.name {
                    if function_imports.insert(name, index).is_some() {
                        return Err(format!("duplicate generated function import '{name}'"));
                    }
                }
            }
            FormKind::Element => elements.push(index),
            FormKind::Export => {
                if let Some(name) = referenced_symbol_after(&form.tokens, "func") {
                    roots.push_back(name);
                }
                exported_table |= referenced_symbol_after(&form.tokens, "table").is_some();
            }
            FormKind::Start => {
                if let Some(name) = form.tokens.iter().copied().find(|token| is_symbol(token)) {
                    roots.push_back(name);
                }
            }
            FormKind::Type | FormKind::Global | FormKind::Table | FormKind::Other => {}
        }
    }

    let mut reachable_functions = HashSet::new();
    let mut needs_indirect_table = exported_table;
    let mut element_roots_added = false;
    loop {
        while let Some(name) = roots.pop_front() {
            if !reachable_functions.insert(name) {
                continue;
            }
            let Some(index) = functions.get(name).copied() else {
                continue;
            };
            let form = &forms[index];
            needs_indirect_table |= form.tokens.contains(&"call_indirect");
            for dependency in referenced_function_symbols(&form.tokens) {
                roots.push_back(dependency);
            }
        }

        if needs_indirect_table && !element_roots_added {
            for index in &elements {
                for function in element_function_symbols(&forms[*index].tokens) {
                    roots.push_back(function);
                }
            }
            element_roots_added = true;
            continue;
        }
        break;
    }

    let mut keep = vec![true; forms.len()];
    let mut report = ReleaseOptimizationReport::default();
    for (index, form) in forms.iter().enumerate() {
        match form.kind {
            FormKind::Function
                if form
                    .name
                    .is_some_and(|name| !reachable_functions.contains(name)) =>
            {
                keep[index] = false;
                report.removed_functions += 1;
            }
            FormKind::FunctionImport
                if form
                    .name
                    .is_some_and(|name| !reachable_functions.contains(name)) =>
            {
                keep[index] = false;
                report.removed_function_imports += 1;
            }
            FormKind::Table if !needs_indirect_table => {
                keep[index] = false;
                report.removed_tables += 1;
            }
            FormKind::Element if !needs_indirect_table => {
                keep[index] = false;
                report.removed_elements += 1;
            }
            _ => {}
        }
    }

    let used_types = referenced_symbols_in_kept_forms(&forms, &keep, "type", FormKind::Type);
    for (index, form) in forms.iter().enumerate() {
        if keep[index]
            && form.kind == FormKind::Type
            && form.name.is_some_and(|name| !used_types.contains(name))
        {
            keep[index] = false;
            report.removed_types += 1;
        }
    }

    let mut used_globals =
        referenced_symbols_in_kept_forms(&forms, &keep, "global.get", FormKind::Global);
    used_globals.extend(referenced_symbols_in_kept_forms(
        &forms,
        &keep,
        "global.set",
        FormKind::Global,
    ));
    for form in &forms {
        if form.kind == FormKind::Export {
            if let Some(name) = referenced_symbol_after(&form.tokens, "global") {
                used_globals.insert(name);
            }
        }
    }

    loop {
        let mut changed = false;
        for form in &forms {
            if form.kind != FormKind::Global
                || !form.name.is_some_and(|name| used_globals.contains(name))
            {
                continue;
            }
            for keyword in ["global.get", "global.set"] {
                for dependency in symbols_after(&form.tokens, keyword) {
                    changed |= used_globals.insert(dependency);
                }
            }
        }
        if !changed {
            break;
        }
    }

    for (index, form) in forms.iter().enumerate() {
        if keep[index]
            && form.kind == FormKind::Global
            && form.name.is_some_and(|name| !used_globals.contains(name))
        {
            keep[index] = false;
            report.removed_globals += 1;
        }
    }

    let mut optimized = String::with_capacity(wat.len());
    let mut cursor = 0;
    for (form, keep_form) in forms.iter().zip(keep) {
        if !keep_form {
            optimized.push_str(&wat[cursor..form.start]);
            cursor = form.end;
        }
    }
    optimized.push_str(&wat[cursor..]);
    Ok((optimized, report))
}

fn referenced_symbols_in_kept_forms<'a>(
    forms: &'a [TopLevelForm<'a>],
    keep: &[bool],
    keyword: &str,
    excluded_kind: FormKind,
) -> HashSet<&'a str> {
    let mut symbols = HashSet::new();
    for (index, form) in forms.iter().enumerate() {
        if keep[index] && form.kind != excluded_kind {
            symbols.extend(symbols_after(&form.tokens, keyword));
        }
    }
    symbols
}

fn referenced_function_symbols<'a>(tokens: &[&'a str]) -> Vec<&'a str> {
    ["call", "return_call", "ref.func"]
        .into_iter()
        .flat_map(|keyword| symbols_after(tokens, keyword))
        .collect()
}

fn element_function_symbols<'a>(tokens: &[&'a str]) -> Vec<&'a str> {
    let Some(function_index) = tokens.iter().position(|token| *token == "func") else {
        return Vec::new();
    };
    tokens[function_index + 1..]
        .iter()
        .copied()
        .filter(|token| is_symbol(token))
        .collect()
}

fn symbols_after<'a>(tokens: &[&'a str], keyword: &str) -> Vec<&'a str> {
    tokens
        .windows(2)
        .filter_map(|pair| (pair[0] == keyword && is_symbol(pair[1])).then_some(pair[1]))
        .collect()
}

fn referenced_symbol_after<'a>(tokens: &[&'a str], keyword: &str) -> Option<&'a str> {
    tokens
        .windows(2)
        .find_map(|pair| (pair[0] == keyword && is_symbol(pair[1])).then_some(pair[1]))
}

fn contains_inline_export(tokens: &[&str]) -> bool {
    tokens.iter().skip(2).any(|token| *token == "export")
}

fn is_symbol(token: &str) -> bool {
    token.starts_with('$') && token.len() > 1
}

fn top_level_forms(wat: &str) -> Result<Vec<TopLevelForm<'_>>, String> {
    let bytes = wat.as_bytes();
    let mut forms = Vec::new();
    let mut depth = 0usize;
    let mut form_start = None;
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment_depth = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment_depth > 0 {
            if byte == b'(' && next == Some(b';') {
                block_comment_depth += 1;
                index += 2;
            } else if byte == b';' && next == Some(b')') {
                block_comment_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b';' && next == Some(b';') {
            line_comment = true;
            index += 2;
            continue;
        }
        if byte == b'(' && next == Some(b';') {
            block_comment_depth = 1;
            index += 2;
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'(' => {
                if depth == 1 {
                    form_start = Some(index);
                }
                depth += 1;
            }
            b')' => {
                if depth == 0 {
                    return Err("generated WAT has an unmatched ')'".to_string());
                }
                depth -= 1;
                if depth == 1 {
                    let start = form_start.take().ok_or_else(|| {
                        "generated WAT lost a top-level form boundary".to_string()
                    })?;
                    let end = index + 1;
                    let text = &wat[start..end];
                    let tokens = wat_tokens(text);
                    let (kind, name) = classify_form(&tokens);
                    forms.push(TopLevelForm {
                        start,
                        end,
                        tokens,
                        kind,
                        name,
                    });
                }
            }
            _ => {}
        }
        index += 1;
    }

    if in_string || block_comment_depth > 0 || depth != 0 {
        return Err("generated WAT has an unterminated string, comment, or form".to_string());
    }
    if forms.is_empty() || !wat.trim_start().starts_with("(module") {
        return Err("generated WAT is not a module".to_string());
    }
    Ok(forms)
}

fn classify_form<'a>(tokens: &[&'a str]) -> (FormKind, Option<&'a str>) {
    let Some(first) = tokens.first().copied() else {
        return (FormKind::Other, None);
    };
    match first {
        "func" => (
            FormKind::Function,
            tokens
                .iter()
                .skip(1)
                .copied()
                .find(|token| is_symbol(token)),
        ),
        "import" => {
            let function_name = referenced_symbol_after(tokens, "func");
            if function_name.is_some() {
                (FormKind::FunctionImport, function_name)
            } else {
                (FormKind::Other, None)
            }
        }
        "type" => (
            FormKind::Type,
            tokens
                .iter()
                .skip(1)
                .copied()
                .find(|token| is_symbol(token)),
        ),
        "global" => (
            FormKind::Global,
            tokens
                .iter()
                .skip(1)
                .copied()
                .find(|token| is_symbol(token)),
        ),
        "table" => (FormKind::Table, None),
        "elem" => (FormKind::Element, None),
        "export" => (FormKind::Export, None),
        "start" => (FormKind::Start, None),
        _ => (FormKind::Other, None),
    }
}

fn wat_tokens(wat: &str) -> Vec<&str> {
    let bytes = wat.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() || matches!(bytes[index], b'(' | b')') {
            index += 1;
            continue;
        }
        if bytes[index] == b';' && bytes.get(index + 1) == Some(&b';') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index += 2;
                } else if bytes[index] == b'"' {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            continue;
        }
        let start = index;
        while index < bytes.len()
            && !bytes[index].is_ascii_whitespace()
            && !matches!(bytes[index], b'(' | b')' | b'"')
        {
            index += 1;
        }
        if start != index {
            tokens.push(&wat[start..index]);
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_dce_keeps_transitive_calls_and_removes_unused_runtime() {
        let wat = r#"(module
  (type $used_type (func (param i32)))
  (type $unused_type (func))
  (import "host" "used" (func $used_import))
  (import "host" "unused" (func $unused_import))
  (global $used_global (mut i32) (i32.const 0))
  (global $unused_global (mut i32) (i32.const 0))
  (func $entry (export "entry") (type $used_type)
    global.get $used_global
    call $helper)
  (func $helper
    call $used_import)
  (func $unused
    call $unused_import
    global.get $unused_global)
)"#;

        let (optimized, report) = eliminate_unreachable(wat).unwrap();
        wat::parse_str(&optimized).unwrap();
        assert!(optimized.contains("(func $entry"));
        assert!(optimized.contains("(func $helper"));
        assert!(optimized.contains("(func $used_import"));
        assert!(optimized.contains("(global $used_global"));
        assert!(optimized.contains("(type $used_type"));
        assert!(!optimized.contains("$unused_import"));
        assert!(!optimized.contains("$unused_global"));
        assert!(!optimized.contains("$unused_type"));
        assert!(!optimized.contains("(func $unused"));
        assert_eq!(report.removed_functions, 1);
        assert_eq!(report.removed_function_imports, 1);
        assert_eq!(report.removed_globals, 1);
        assert_eq!(report.removed_types, 1);
    }

    #[test]
    fn release_dce_keeps_element_functions_only_for_reachable_indirect_calls() {
        let with_indirect = r#"(module
  (type $callback (func (result i32)))
  (func $entry (export "entry") (result i32)
    i32.const 0
    call_indirect (type $callback))
  (func $callback_impl (result i32) i32.const 1)
  (table 1 funcref)
  (elem (i32.const 0) func $callback_impl)
)"#;
        let (optimized, _) = eliminate_unreachable(with_indirect).unwrap();
        wat::parse_str(&optimized).unwrap();
        assert!(optimized.contains("$callback_impl"));
        assert!(optimized.contains("(table"));
        assert!(optimized.contains("(elem"));

        let without_indirect = r#"(module
  (func $entry (export "entry") (result i32) i32.const 1)
  (func $unused_callback (result i32) i32.const 2)
  (table 1 funcref)
  (elem (i32.const 0) func $unused_callback)
)"#;
        let (optimized, report) = eliminate_unreachable(without_indirect).unwrap();
        wat::parse_str(&optimized).unwrap();
        assert!(!optimized.contains("$unused_callback"));
        assert!(!optimized.contains("(table"));
        assert!(!optimized.contains("(elem"));
        assert_eq!(report.removed_tables, 1);
        assert_eq!(report.removed_elements, 1);
    }

    #[test]
    fn release_dce_is_deterministic() {
        let wat = "(module (func $entry (export \"entry\")) (func $unused))";
        let (first, _) = eliminate_unreachable(wat).unwrap();
        let (second, _) = eliminate_unreachable(wat).unwrap();
        assert_eq!(first, second);
    }
}
