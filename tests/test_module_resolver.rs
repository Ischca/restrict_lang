use restrict_lang::ast::{ExprKind, ImportItems, PipeTarget, TopDecl, Type};
#[cfg(not(target_arch = "wasm32"))]
use restrict_lang::dev_tools::{DevTools, DiagnosticSeverity};
use restrict_lang::module::{
    parse_module_source_key, resolve_program_imports_for_file,
    resolve_program_imports_with_module_source_map, ModuleResolver,
};
use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn parse_complete(source: &str) -> restrict_lang::ast::Program {
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely, remaining: {remaining:?}"
    );
    program
}

fn instantiate_wat(label: &str, wat: &str) -> (Store<()>, Instance) {
    let wasm = wat::parse_str(wat).unwrap_or_else(|err| {
        panic!("{label} generated invalid WAT: {err}\n\n{wat}");
    });

    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("{label} generated invalid Wasm binary: {err}\n\n{wat}");
        });

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..]).unwrap_or_else(|err| {
        panic!("{label} generated Wasm that wasmi cannot load: {err}\n\n{wat}");
    });
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);

    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "fd_write",
            |_caller: Caller<'_, ()>,
             _fd: i32,
             _iovs: i32,
             _iovs_len: i32,
             _nwritten: i32|
             -> i32 { 0 },
        )
        .expect("fd_write stub should be registered");
    linker
        .func_wrap(
            "wasi_snapshot_preview1",
            "proc_exit",
            |_caller: Caller<'_, ()>, _code: i32| {},
        )
        .expect("proc_exit stub should be registered");

    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|err| {
            panic!("{label} generated Wasm that wasmi cannot instantiate: {err}\n\n{wat}");
        });

    (store, instance)
}

fn temp_module_dir(name: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "restrict_lang_{}_{}_{}",
        name,
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("module temp dir should be created");
    dir
}

fn internal_module_name(module_path: &[&str], name: &str) -> String {
    let mut mangled = String::from("__rl$mod");
    for part in module_path.iter().copied().chain([name]) {
        mangled.push('_');
        mangled.push_str(&part.len().to_string());
        mangled.push('_');
        mangled.push_str(part);
    }
    mangled
}

struct RemoveFileOnDrop(PathBuf);

impl Drop for RemoveFileOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn resolver_collects_named_function_exports() {
    let dir = temp_module_dir("named_exports");
    fs::write(
        dir.join("release.rl"),
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    value
}
"#,
    )
    .expect("module source should be written");

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    resolver
        .resolve_module(&["release".to_string()])
        .expect("module should resolve");

    let imported = resolver
        .get_imported_items(
            &["release".to_string()],
            &ImportItems::Named(vec!["public_score".to_string()]),
        )
        .expect("named export should be available");

    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].0, "public_score");
    assert!(matches!(imported[0].1, TopDecl::Function(_)));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_imports_exported_records_as_source_level_types() {
    let dir = temp_module_dir("record_exports");
    fs::write(
        dir.join("release.rl"),
        r#"
export record ReleaseSlice {
    score: Int32
}

export fun public_score: (slice: ReleaseSlice) -> Int32 = {
    slice.score
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{ReleaseSlice, public_score}

fun main: () -> Int32 = {
    val slice = ReleaseSlice { score: 41 }
    slice |> public_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("record and function imports should resolve");

    assert!(resolved.imports.is_empty());
    assert!(matches!(
        resolved.declarations.first(),
        Some(TopDecl::Export(export))
            if matches!(export.item.as_ref(), TopDecl::Record(record) if record.name == "ReleaseSlice")
    ));

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("source-level exported record should type check after import");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("resolved record import should generate WAT");
    assert!(
        !wat.contains("(export \"ReleaseSlice\""),
        "record source import must not imply a host-visible Wasm ABI:\n{wat}"
    );
    assert!(wat.contains("call $public_score"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn complex_top_level_binding_export_returns_error_instead_of_panicking() {
    let dir = temp_module_dir("complex_binding_export");
    fs::write(
        dir.join("bad_export.rl"),
        r#"
record Pair {
    left: Int32,
    right: Int32
}

export val Pair { left, right } = Pair { left: 1, right: 2 }
"#,
    )
    .expect("module source should be written");

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let err = resolver
        .resolve_module(&["bad_export".to_string()])
        .expect_err("complex binding export should be a resolver error");

    assert!(
        err.to_string()
            .contains("Complex top-level binding exports are not supported yet"),
        "error should explain the unsupported export shape, got: {err}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_expands_named_imports_before_type_checking_and_codegen() {
    let dir = temp_module_dir("expand_named_imports");
    fs::write(
        dir.join("release.rl"),
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{public_score}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("imports should resolve");

    assert!(resolved.imports.is_empty());
    assert!(matches!(
        resolved.declarations.first(),
        Some(TopDecl::Function(fun)) if fun.name == "public_score"
    ));

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("resolved program should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("resolved program should generate WAT");
    assert!(wat.contains("(func $public_score"));
    assert!(wat.contains("call $public_score"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolve_program_imports_for_file_uses_source_parent_directory() {
    let dir = temp_module_dir("source_parent_imports");
    fs::write(
        dir.join("release.rl"),
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{public_score}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    );

    let resolved = resolve_program_imports_for_file(root, &dir.join("app.rl"))
        .expect("source-relative import should resolve");

    assert!(resolved.imports.is_empty());

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("source-relative imports should type check");

    let _ = fs::remove_dir_all(dir);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dev_tools_lsp_diagnostics_resolve_imports_from_source_path() {
    let dir = temp_module_dir("dev_tools_import_diagnostics");
    fs::write(
        dir.join("release.rl"),
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("module source should be written");

    let source = r#"
import release.{public_score}

fun main: () -> Int32 = {
    41 |> public_score
}
"#;

    let diagnostics = DevTools::lsp_diagnostics_for_path(source, &dir.join("app.rl"));

    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.severity, DiagnosticSeverity::Error)),
        "source-relative import should not produce diagnostics: {diagnostics:?}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dev_tools_type_diagnostics_use_user_facing_display_text() {
    let source = r#"
fun main: () -> Int32 = {
    true
}
"#;

    let diagnostics = DevTools::lsp_diagnostics(source);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, DiagnosticSeverity::Error))
        .expect("type error should produce a diagnostic");

    assert!(
        diagnostic
            .message
            .contains("Type mismatch: expected Int32, found Boolean"),
        "diagnostic should use TypeError Display text, got: {}",
        diagnostic.message
    );
    assert!(
        !diagnostic.message.contains("TypeMismatch {"),
        "diagnostic should not expose Rust debug enum formatting, got: {}",
        diagnostic.message
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dev_tools_type_diagnostics_preserve_inference_context_and_binding_position() {
    let source = r#"
fun main: () -> Int32 = {
    val items = [];
    0
}
"#;

    let diagnostics = DevTools::lsp_diagnostics(source);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, DiagnosticSeverity::Error))
        .expect("unresolved collection binding should produce a diagnostic");

    assert!(
        diagnostic
            .message
            .contains("Cannot infer type for binding 'items'"),
        "diagnostic should keep the binding context, got: {}",
        diagnostic.message
    );
    assert!(
        diagnostic
            .message
            .contains("empty list requires an expected List type"),
        "diagnostic should keep the empty-list hint, got: {}",
        diagnostic.message
    );
    assert!(!diagnostic.message.contains("?0"));
    assert_eq!(diagnostic.line, 2);
    assert_eq!(diagnostic.column, 8);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn dev_tools_unresolved_builtin_projection_diagnostic_hides_internals() {
    let source = r#"
fun main: () -> Int32 = {
    val apply_map = map;
    0
}
"#;

    let diagnostics = DevTools::lsp_diagnostics(source);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, DiagnosticSeverity::Error))
        .expect("unresolved builtin projection should produce a diagnostic");

    assert!(
        diagnostic
            .message
            .contains("Cannot infer type for binding 'apply_map'"),
        "diagnostic should keep the binding context, got: {}",
        diagnostic.message
    );
    for internal in ["?0", "InferVar", "TypeVarId", "Projection"] {
        assert!(
            !diagnostic.message.contains(internal),
            "diagnostic should not expose type inference internals ({internal}), got: {}",
            diagnostic.message
        );
    }
    assert_eq!(diagnostic.line, 2);
    assert_eq!(diagnostic.column, 8);
}

#[test]
fn module_source_key_accepts_dotted_or_file_like_names() {
    assert_eq!(
        parse_module_source_key("modules.release_policy").expect("dotted key should parse"),
        vec!["modules".to_string(), "release_policy".to_string()]
    );
    assert_eq!(
        parse_module_source_key("modules/release_scores.rl").expect("file-like key should parse"),
        vec!["modules".to_string(), "release_scores".to_string()]
    );
}

#[test]
fn parser_rejects_unimplemented_string_import_alias_syntax() {
    let err = parse_program(
        r#"
import "std/io" as io

fun main: () -> Int32 = {
    1
}
"#,
    )
    .expect_err("string import aliases are outside the v0.0.1 module surface");

    assert!(
        format!("{err:?}").contains("string import paths and import aliases are unsupported"),
        "parse error should explain the v0.0.1 import boundary, got: {err:?}"
    );
}

#[test]
fn parser_rejects_unimplemented_dotted_import_alias_syntax() {
    let err = parse_program(
        r#"
import release.policy as policy

fun main: () -> Int32 = {
    1
}
"#,
    )
    .expect_err("import aliases are outside the v0.0.1 module surface");

    assert!(
        format!("{err:?}").contains("string import paths and import aliases are unsupported"),
        "parse error should explain the v0.0.1 import boundary, got: {err:?}"
    );
}

#[test]
fn parser_rejects_unimplemented_re_export_syntax() {
    let err = parse_program(
        r#"
export import release.policy.{score}

fun main: () -> Int32 = {
    1
}
"#,
    )
    .expect_err("re-exports are outside the v0.0.1 module surface");

    assert!(
        format!("{err:?}").contains("re-exports are unsupported in v0.0.1"),
        "parse error should explain the v0.0.1 re-export boundary, got: {err:?}"
    );
}

#[test]
fn resolver_rejects_std_aggregator_import_with_v001_message() {
    let root = parse_complete(
        r#"
import std.prelude

fun main: () -> Int32 = {
    1 |> identity
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    let err = resolver
        .resolve_program_imports(root)
        .expect_err("std source aggregators are outside the v0.0.1 module surface");

    assert!(
        err.to_string()
            .contains("standard-library source imports are unsupported in v0.0.1"),
        "resolver error should explain the std import boundary, got: {err}"
    );
}

#[test]
fn resolver_expands_virtual_module_sources_for_browser_like_hosts() {
    let root = parse_complete(
        r#"
import modules.release_policy.{public_score}

fun score: (value: Int32) -> Int32 = {
    value - 1
}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "modules/release_policy.rl".to_string(),
        r#"
import modules.release_scores.{score}

export fun public_score: (value: Int32) -> Int32 = {
    value |> score
}
"#
        .to_string(),
    );
    sources.insert(
        "modules.release_scores".to_string(),
        r#"
export fun score: (value: Int32) -> Int32 = {
    value + 1
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("virtual module imports should resolve");

    assert!(resolved.imports.is_empty());

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("virtual-module-resolved program should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("virtual-module-resolved program should generate WAT");
    let internal_score = internal_module_name(&["modules", "release_scores"], "score");
    assert!(wat.contains(&format!("(func ${internal_score}")));
    assert!(wat.contains("(func $score"));
    assert!(wat.contains(&format!("call ${internal_score}")));
    assert!(wat.contains("call $public_score"));
}

#[test]
fn resolver_imports_generic_function_with_inferred_return() {
    let root = parse_complete(
        r#"
import release.{wrap}

fun main: () -> Option<Float64> = {
    1.5 |> wrap
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "release".to_string(),
        r#"
export fun wrap: <T>(value: T) = {
    Some(value)
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("generic inferred export should resolve");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("generic inferred export should type check across module boundary");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("generic inferred export should monomorphize after import");

    assert!(
        wat.contains("$wrap__Float64"),
        "imported generic function should be specialized from the call site:\n{wat}"
    );
    assert!(
        wat.contains("call $wrap__Float64"),
        "main should call the specialized imported generic function:\n{wat}"
    );
}

#[test]
fn resolved_source_imports_execute_in_wasm_runtime() {
    let root = parse_complete(
        r#"
import modules.policy.{ReleaseInput, evaluate_release}
import modules.scores.*
import modules.generics

export fun imported_release_score: (manual_owner_id: Int32) -> Int32 = {
    val manual_owner: Option<Int32> = manual_owner_id > 0 then {
        Some(manual_owner_id)
    } else {
        None
    };
    val selected_owner = (manual_owner, 102) choose_or;
    val base_score = (5, 10) sum_score;
    val input = ReleaseInput {
        signal: 1,
        owner: selected_owner,
        base_score: base_score
    };

    input |> evaluate_release
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "modules.scores".to_string(),
        r#"
export fun score_signal: (signal: Int32) -> Int32 = {
    signal * 2
}

export fun sum_score: (left: Int32, right: Int32) -> Int32 = {
    left + right
}
"#
        .to_string(),
    );
    sources.insert(
        "modules.generics".to_string(),
        r#"
export fun choose_or: <T>(preferred: Option<T>, fallback: T) -> T = {
    preferred match {
        Some(value) => {
            value
        }
        None => {
            fallback
        }
    }
}
"#
        .to_string(),
    );
    sources.insert(
        "modules.policy".to_string(),
        r#"
import modules.scores.{score_signal}

export record ReleaseInput {
    signal: Int32,
    owner: Int32,
    base_score: Int32
}

export fun evaluate_release: (input: ReleaseInput) -> Int32 = {
    val ReleaseInput {
        signal,
        owner,
        base_score
    } = input;
    val signal_score = signal |> score_signal;

    signal_score + owner + base_score
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("named, wildcard, and whole-module imports should resolve");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("resolved source imports should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("resolved source imports should generate WAT");

    assert!(
        !wat.contains("(export \"ReleaseInput\""),
        "imported source records must not become host-visible Wasm exports:\n{wat}"
    );
    assert!(
        !wat.contains("(export \"choose_or\""),
        "imported generic helpers should stay off the host-visible export surface:\n{wat}"
    );
    assert!(
        wat.contains("(func $choose_or__Int32"),
        "whole-module generic import should specialize at the root call site:\n{wat}"
    );

    let (mut store, instance) = instantiate_wat("source import runtime smoke", &wat);
    let imported_release_score = instance
        .get_typed_func::<i32, i32>(&store, "imported_release_score")
        .expect("primitive runtime smoke export should be host-callable");

    assert_eq!(
        imported_release_score
            .call(&mut store, 10)
            .expect("manual owner path should execute"),
        27
    );
    assert_eq!(
        imported_release_score
            .call(&mut store, 0)
            .expect("fallback owner path should execute"),
        119
    );
}

#[test]
fn resolver_keeps_module_private_helpers_internal() {
    let dir = temp_module_dir("private_helpers");
    fs::write(
        dir.join("release.rl"),
        r#"
fun score: (value: Int32) -> Int32 = {
    value + 1
}

export fun public_score: (value: Int32) -> Int32 = {
    value |> score
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{public_score}

fun score: (value: Int32) -> Int32 = {
    value - 1
}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("imports should resolve without leaking private helper names");

    let internal_name = internal_module_name(&["release"], "score");
    assert!(
        resolved
            .declarations
            .iter()
            .any(|decl| { matches!(decl, TopDecl::Function(fun) if fun.name == internal_name) }),
        "private helper should be emitted under an internal module name"
    );

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("resolved program should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("resolved program should generate WAT");
    assert!(wat.contains(&format!("(func ${internal_name}")));
    assert!(wat.contains("(func $score"));
    assert!(wat.contains(&format!("call ${internal_name}")));
    assert!(wat.contains("call $public_score"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_includes_nested_imports_as_internal_dependencies() {
    let dir = temp_module_dir("nested_imports");
    fs::write(
        dir.join("score_util.rl"),
        r#"
export fun score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("dependency module source should be written");
    fs::write(
        dir.join("release.rl"),
        r#"
import score_util.{score}

export fun public_score: (value: Int32) -> Int32 = {
    value |> score
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{public_score}

fun score: (value: Int32) -> Int32 = {
    value - 1
}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("nested imports should resolve without leaking dependency names");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("resolved program should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("resolved program should generate WAT");
    let internal_score = internal_module_name(&["score_util"], "score");
    assert!(wat.contains(&format!("(func ${internal_score}")));
    assert!(wat.contains("(func $score"));
    assert!(wat.contains(&format!("call ${internal_score}")));
    assert!(wat.contains("call $public_score"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_renames_private_context_dependencies() {
    let dir = temp_module_dir("private_context_dependency");
    fs::write(
        dir.join("release.rl"),
        r#"
record PolicyLimits {
    minimum: Int32,
    offset: Int32
}

context Policy {
    limits: PolicyLimits
}

export fun public_score: (value: Int32) -> Int32 = {
    with Policy {
        limits: PolicyLimits {
            minimum: 40,
            offset: 2
        }
    } {
        val PolicyLimits { minimum, offset } = limits;
        val adjusted = value + offset;
        adjusted > minimum then {
            adjusted
        } else {
            minimum
        }
    }
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{public_score}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("private context dependency should resolve");

    let internal_policy = internal_module_name(&["release"], "Policy");
    let internal_limits = internal_module_name(&["release"], "PolicyLimits");
    assert!(
        resolved.declarations.iter().any(|decl| {
            matches!(decl, TopDecl::Context(context) if context.name == internal_policy)
        }),
        "private context should be emitted under an internal module name"
    );
    assert!(
        resolved.declarations.iter().any(|decl| {
            matches!(decl, TopDecl::Record(record) if record.name == internal_limits)
        }),
        "private context field record should be emitted under an internal module name"
    );
    assert!(
        resolved.declarations.iter().any(|decl| {
            matches!(
                decl,
                TopDecl::Context(context)
                    if context.fields.iter().any(|field| {
                        matches!(
                            &field.ty,
                            Type::Named(name) if name == &internal_limits
                        )
                    })
            )
        }),
        "private context field type should be renamed with the private record"
    );

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("resolved program with private context should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("resolved program with private context should generate WAT");
    assert!(wat.contains(&format!(";; With context: {internal_policy}")));
    assert!(wat.contains("call $public_score"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_preserves_private_impl_dependencies() {
    let dir = temp_module_dir("private_impl_dependency");
    fs::write(
        dir.join("release.rl"),
        r#"
record Signal {
    severity: Int32,
    confidence: Int32
}

impl Signal {
    fun risk_score: (self: Signal) -> Int32 = {
        self.severity + self.confidence
    }
}

impl Signal {
    fun risk_bucket: (self: Signal) -> Int32 = {
        self.severity > 10 then {
            1
        } else {
            0
        }
    }
}

export fun public_score: (severity: Int32, confidence: Int32) -> Int32 = {
    val signal = Signal {
        severity: severity,
        confidence: confidence
    };
    val bucket_signal = Signal {
        severity: severity,
        confidence: confidence
    };

    val score = (signal) risk_score;
    val bucket = (bucket_signal) risk_bucket;
    score + bucket
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{public_score}

fun main: () -> Int32 = {
    (20, 7) public_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("private impl dependency should resolve");

    let internal_signal = internal_module_name(&["release"], "Signal");
    assert!(
        resolved.declarations.iter().any(|decl| {
            matches!(decl, TopDecl::Record(record) if record.name == internal_signal)
        }),
        "private method receiver record should be emitted under an internal module name"
    );
    assert!(
        resolved
            .declarations
            .iter()
            .filter(|decl| {
                matches!(decl, TopDecl::Impl(impl_block) if impl_block.target == internal_signal)
            })
            .count()
            >= 2,
        "private impl blocks should not be deduplicated away by their receiver record"
    );

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("resolved program with private impl should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("resolved program with private impl should generate WAT");
    assert!(wat.contains(&format!("(func ${internal_signal}_risk_score")));
    assert!(wat.contains(&format!("(func ${internal_signal}_risk_bucket")));
    assert!(wat.contains(&format!("call ${internal_signal}_risk_score")));
    assert!(wat.contains(&format!("call ${internal_signal}_risk_bucket")));
    assert!(wat.contains("call $public_score"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_expands_wildcard_imports_deterministically() {
    let dir = temp_module_dir("expand_wildcard_imports");
    fs::write(
        dir.join("policy.rl"),
        r#"
export fun z_score: (value: Int32) -> Int32 = {
    value + 2
}

export fun a_score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import policy.*

fun main: () -> Int32 = {
    val base = 10 |> a_score;
    base |> z_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("wildcard import should resolve");

    let imported_names = resolved
        .declarations
        .iter()
        .take(2)
        .map(|decl| match decl {
            TopDecl::Function(fun) => fun.name.as_str(),
            _ => "<non-function>",
        })
        .collect::<Vec<_>>();
    assert_eq!(imported_names, vec!["a_score", "z_score"]);

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("wildcard-resolved program should type check");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_expands_whole_module_imports_deterministically() {
    let dir = temp_module_dir("expand_whole_module_imports");
    fs::write(
        dir.join("policy.rl"),
        r#"
export fun z_score: (value: Int32) -> Int32 = {
    value + 2
}

export fun a_score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import policy

fun main: () -> Int32 = {
    val base = 10 |> a_score;
    base |> z_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("whole-module import should resolve");

    let imported_names = resolved
        .declarations
        .iter()
        .take(2)
        .map(|decl| match decl {
            TopDecl::Function(fun) => fun.name.as_str(),
            _ => "<non-function>",
        })
        .collect::<Vec<_>>();
    assert_eq!(imported_names, vec!["a_score", "z_score"]);

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("whole-module-resolved program should type check");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_rejects_import_that_collides_with_root_declaration() {
    let dir = temp_module_dir("import_collision");
    fs::write(
        dir.join("release.rl"),
        r#"
export fun score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("module source should be written");

    let root = parse_complete(
        r#"
import release.{score}

fun score: (value: Int32) -> Int32 = {
    value
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    let err = resolver
        .resolve_program_imports(root)
        .expect_err("colliding import should be rejected");

    assert!(
        err.to_string()
            .contains("Import name collision for 'score'"),
        "error should explain the name collision, got: {err}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn import_resolution_renumbers_node_ids_densely() {
    let root = parse_complete(
        r#"
import release_math.{bump}

fun main: () -> Int32 = {
    1 |> bump
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "release_math".to_string(),
        r#"
export fun bump: (value: Int32) -> Int32 = {
    value + 1
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("virtual module import should resolve");

    // Imported declarations were numbered per source file; the spliced
    // program must come back as one dense, program-wide id space.
    let ids = restrict_lang::ast::collect_node_ids(&resolved);
    assert!(!ids.is_empty());
    let expected = (0..ids.len() as u32)
        .map(restrict_lang::ast::NodeId)
        .collect::<Vec<_>>();
    assert_eq!(ids, expected);
}

#[test]
fn import_renaming_reaches_cast_and_range_subtrees() {
    let root = parse_complete(
        r#"
import release_math.{widen}

fun main: () -> Int64 = {
    3 |> widen
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "release_math".to_string(),
        r#"
fun pad: (value: Int32) -> Int32 = {
    value + 1
}

fun span: (value: Int32) -> Range<Int32> = {
    [1..(value |> pad)]
}

export fun widen: (value: Int32) -> Int64 = {
    (value |> pad) as Int64
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("virtual module import should resolve");

    // Module-local declarations are mangled during splicing. References that
    // live only inside cast operands or range endpoints must follow that
    // renaming; a dangling unmangled name is silently rebound by the
    // pipe-to-binding fallback rather than rejected, so pin the resolved AST
    // structurally instead of via type checking.
    let pad_name = resolved
        .declarations
        .iter()
        .find_map(|decl| match decl {
            TopDecl::Function(func) if func.name != "pad" && func.name.contains("pad") => {
                Some(func.name.clone())
            }
            _ => None,
        })
        .expect("module-local pad declaration should be spliced under a mangled name");

    let find_function = |name: &str| {
        resolved.declarations.iter().find_map(|decl| match decl {
            TopDecl::Function(func) if func.name.contains(name) => Some(func),
            _ => None,
        })
    };

    let widen = find_function("widen").expect("widen should be spliced");
    let widen_result = widen
        .body
        .expr
        .as_ref()
        .expect("widen body should end in an expression");
    let ExprKind::Cast(cast) = &widen_result.kind else {
        panic!("widen body should end in a cast");
    };
    let ExprKind::Pipe(pipe) = &cast.expr.kind else {
        panic!("cast operand should be a pipe");
    };
    let PipeTarget::Ident(target) = &pipe.target else {
        panic!("pipe target should be an identifier");
    };
    assert_eq!(
        target, &pad_name,
        "cast operands must follow module renaming"
    );

    let span = find_function("span").expect("span should be spliced");
    let span_result = span
        .body
        .expr
        .as_ref()
        .expect("span body should end in an expression");
    let ExprKind::RangeLit(range) = &span_result.kind else {
        panic!("span body should end in a range literal");
    };
    let ExprKind::Pipe(pipe) = &range.end.kind else {
        panic!("range end should be a pipe");
    };
    let PipeTarget::Ident(target) = &pipe.target else {
        panic!("pipe target should be an identifier");
    };
    assert_eq!(
        target, &pad_name,
        "range endpoints must follow module renaming"
    );
}

#[test]
fn split_named_imports_preserve_one_nominal_record_identity() {
    let root = parse_complete(
        r#"
import release_model.{ReleaseSlice}
import release_model.{make_slice}

export fun split_import_score: () -> Int32 = {
    val ReleaseSlice { base, bonus } = (40, 2) make_slice;
    base + bonus
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "release_model".to_string(),
        r#"
export record ReleaseSlice {
    base: Int32,
    bonus: Int32
}

export fun make_slice: (base: Int32, bonus: Int32) -> ReleaseSlice = {
    ReleaseSlice {
        base: base,
        bonus: bonus
    }
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("split named imports should resolve to one module identity");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("the imported record and factory signature should share one nominal type");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("split named imports should generate WAT");
    let (mut store, instance) = instantiate_wat("split named record imports", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "split_import_score")
        .expect("split import regression export should be host-callable");

    assert_eq!(
        score
            .call(&mut store, ())
            .expect("split import regression export should execute"),
        42
    );
}

#[test]
fn direct_and_transitive_imports_share_one_nominal_record_identity() {
    let root = parse_complete(
        r#"
import release_model.{ReleaseSlice}
import release_policy.{evaluate_slice}

export fun transitive_import_score: () -> Int32 = {
    val slice = ReleaseSlice {
        base: 40,
        bonus: 2
    };
    slice |> evaluate_slice
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "release_model".to_string(),
        r#"
export record ReleaseSlice {
    base: Int32,
    bonus: Int32
}
"#
        .to_string(),
    );
    sources.insert(
        "release_policy".to_string(),
        r#"
import release_model.{ReleaseSlice}

export fun evaluate_slice: (slice: ReleaseSlice) -> Int32 = {
    val ReleaseSlice { base, bonus } = slice;
    base + bonus
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("direct and transitive imports should resolve");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("direct and transitive references should share one nominal record type");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("direct and transitive imports should generate WAT");
    let (mut store, instance) = instantiate_wat("direct and transitive record imports", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "transitive_import_score")
        .expect("transitive identity regression export should be host-callable");

    assert_eq!(
        score
            .call(&mut store, ())
            .expect("transitive identity regression export should execute"),
        42
    );
}

#[test]
fn internal_module_names_are_collision_proof_across_path_segments() {
    let root = parse_complete(
        r#"
import left_adapter.{left_score}
import right_adapter.{right_score}

export fun mangling_collision_score: () -> Int32 = {
    val left = () left_score;
    val right = () right_score;
    left + right
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "a_b.c".to_string(),
        r#"
export fun score: () -> Int32 = {
    10
}
"#
        .to_string(),
    );
    sources.insert(
        "a.b_c".to_string(),
        r#"
export fun score: () -> Int32 = {
    20
}
"#
        .to_string(),
    );
    sources.insert(
        "left_adapter".to_string(),
        r#"
import a_b.c.{score}

export fun left_score: () -> Int32 = {
    () score
}
"#
        .to_string(),
    );
    sources.insert(
        "right_adapter".to_string(),
        r#"
import a.b_c.{score}

export fun right_score: () -> Int32 = {
    () score
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("modules whose underscore-joined paths collide should still resolve");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("collision-proof internal module names should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("collision-proof internal module names should generate WAT");
    let (mut store, instance) = instantiate_wat("collision-proof module names", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "mangling_collision_score")
        .expect("mangling regression export should be host-callable");

    assert_eq!(
        score
            .call(&mut store, ())
            .expect("mangling regression export should execute"),
        30,
        "both colliding module paths must retain their own implementation"
    );
}

#[test]
fn internal_module_namespace_cannot_collide_with_source_identifiers() {
    let root = parse_complete(
        r#"
import policy.{public_score}

fun __rl_mod_6_policy_5_score: () -> Int32 = {
    1
}

export fun source_namespace_score: () -> Int32 = {
    val public = () public_score;
    val local = () __rl_mod_6_policy_5_score;
    public + local
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "policy".to_string(),
        r#"
fun score: () -> Int32 = {
    41
}

export fun public_score: () -> Int32 = {
    () score
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("source identifiers must not collide with the internal namespace");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("source/internal namespace separation should type check");
    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("source/internal namespace separation should generate WAT");
    assert!(wat.contains("$__rl$mod_6_policy_5_score"));
    assert!(wat.contains("$__rl_mod_6_policy_5_score"));

    let (mut store, instance) = instantiate_wat("source/internal module namespace", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "source_namespace_score")
        .expect("namespace regression export should be host-callable");
    assert_eq!(
        score
            .call(&mut store, ())
            .expect("namespace regression export should execute"),
        42
    );
}

#[test]
fn failed_module_resolution_does_not_poison_retry_cache() {
    let root = parse_complete(
        r#"
import release_parent.{parent_score}

export fun retry_score: () -> Int32 = {
    () parent_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .try_add_module_source(
            vec!["release_parent".to_string()],
            r#"
import release_child.{child_score}

export fun parent_score: () -> Int32 = {
    () child_score
}
"#
            .to_string(),
        )
        .expect("parent module source should be registered");

    let first_error = resolver
        .resolve_program_imports(root.clone())
        .expect_err("the first resolution should fail while the child is missing");
    assert!(
        first_error.to_string().contains("release_child"),
        "the initial error should identify the missing child module: {first_error}"
    );

    resolver
        .try_add_module_source(
            vec!["release_child".to_string()],
            r#"
export fun child_score: () -> Int32 = {
    42
}
"#
            .to_string(),
        )
        .expect("child module source should be registered");

    let resolved = resolver
        .resolve_program_imports(root)
        .expect("retry should resolve after the missing child is supplied");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("a successful retry should produce a complete program");
}

#[test]
fn cyclic_import_error_reports_the_complete_chain() {
    let root = parse_complete(
        r#"
import cycle_a.{a_score}

fun main: () -> Int32 = {
    () a_score
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "cycle_a".to_string(),
        r#"
import cycle_b.{b_score}

export fun a_score: () -> Int32 = {
    1
}
"#
        .to_string(),
    );
    sources.insert(
        "cycle_b".to_string(),
        r#"
import cycle_c.{c_score}

export fun b_score: () -> Int32 = {
    2
}
"#
        .to_string(),
    );
    sources.insert(
        "cycle_c".to_string(),
        r#"
import cycle_a.{a_score}

export fun c_score: () -> Int32 = {
    3
}
"#
        .to_string(),
    );

    let error = resolve_program_imports_with_module_source_map(root, sources)
        .expect_err("cyclic imports should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("cycle_a -> cycle_b -> cycle_c -> cycle_a"),
        "cycle diagnostic should include the complete import chain, got: {message}"
    );
}

#[test]
fn resolver_rejects_duplicate_exports_in_one_module() {
    let root = parse_complete(
        r#"
import duplicate_policy.{score}

fun main: () -> Int32 = {
    () score
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "duplicate_policy".to_string(),
        r#"
export fun score: () -> Int32 = {
    1
}

export fun score: () -> Int32 = {
    2
}
"#
        .to_string(),
    );

    let error = resolve_program_imports_with_module_source_map(root, sources)
        .expect_err("duplicate exports should be rejected instead of overwritten");
    let message = error.to_string();
    assert!(
        message.to_lowercase().contains("duplicate export")
            && message.contains("score")
            && message.contains("duplicate_policy"),
        "duplicate export diagnostic should name the module and export, got: {message}"
    );
}

#[test]
fn resolver_rejects_duplicate_normalized_virtual_module_sources() {
    let mut resolver = ModuleResolver::new();
    resolver
        .add_module_source_key(
            "policy.rules",
            r#"
export fun score: () -> Int32 = {
    1
}
"#
            .to_string(),
        )
        .expect("the first virtual module spelling should be accepted");

    let error = resolver
        .add_module_source_key(
            "policy/rules.rl",
            r#"
export fun score: () -> Int32 = {
    2
}
"#
            .to_string(),
        )
        .expect_err("equivalent virtual module spellings must not overwrite each other");
    let message = error.to_string();
    assert!(
        message.to_lowercase().contains("duplicate") && message.contains("policy.rules"),
        "duplicate virtual module diagnostic should name its canonical identity, got: {message}"
    );
}

#[test]
fn infallible_module_source_registration_reports_duplicates_during_resolution() {
    let module_path = vec!["release_policy".to_string()];
    let mut resolver = ModuleResolver::new();
    resolver.add_module_source(
        module_path.clone(),
        r#"
export fun score: () -> Int32 = {
    1
}
"#
        .to_string(),
    );
    resolver.add_module_source(
        module_path.clone(),
        r#"
export fun score: () -> Int32 = {
    2
}
"#
        .to_string(),
    );

    let error = resolver
        .resolve_module(&module_path)
        .expect_err("the compatibility API must not silently accept a duplicate identity");
    let message = error.to_string();
    assert!(
        message.to_lowercase().contains("duplicate") && message.contains("release_policy"),
        "deferred duplicate diagnostic should name the module, got: {message}"
    );
}

#[test]
fn checked_module_source_registration_rejects_an_already_resolved_identity() {
    let dir = temp_module_dir("late_module_source_registration");
    fs::write(
        dir.join("release_policy.rl"),
        r#"
export fun score: () -> Int32 = {
    1
}
"#,
    )
    .expect("filesystem module should be written");

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(dir.clone())
        .expect("module search root should be registered");
    resolver
        .resolve_module(&["release_policy".to_string()])
        .expect("filesystem module should resolve");

    let error = resolver
        .try_add_module_source(
            vec!["release_policy".to_string()],
            r#"
export fun score: () -> Int32 = {
    2
}
"#
            .to_string(),
        )
        .expect_err("checked registration must not silently shadow a cached module");
    let message = error.to_string();
    assert!(
        message.contains("already resolved") && message.contains("release_policy"),
        "late registration diagnostic should identify the cached module, got: {message}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn resolver_rejects_ambiguous_files_across_explicit_search_paths() {
    let first = temp_module_dir("ambiguous_module_first");
    let second = temp_module_dir("ambiguous_module_second");
    fs::create_dir_all(first.join("release")).expect("first module namespace should be created");
    fs::create_dir_all(second.join("release")).expect("second module namespace should be created");
    fs::write(
        first.join("release/policy.rl"),
        r#"
export fun score: () -> Int32 = {
    1
}
"#,
    )
    .expect("first ambiguous module should be written");
    fs::write(
        second.join("release/policy.rl"),
        r#"
export fun score: () -> Int32 = {
    2
}
"#,
    )
    .expect("second ambiguous module should be written");

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(first.clone())
        .expect("first module search root should be registered");
    resolver
        .add_search_path(second.clone())
        .expect("second module search root should be registered");
    let error = resolver
        .resolve_module(&["release".to_string(), "policy".to_string()])
        .expect_err("distinct files for one module identity should be ambiguous");
    let message = error.to_string();
    let canonical_first = fs::canonicalize(&first).expect("first root should canonicalize");
    let canonical_second = fs::canonicalize(&second).expect("second root should canonicalize");
    assert!(
        message.to_lowercase().contains("ambiguous")
            && message.contains("release.policy")
            && message.contains(&canonical_first.display().to_string())
            && message.contains(&canonical_second.display().to_string()),
        "ambiguity diagnostic should name the module and both candidates, got: {message}"
    );

    let _ = fs::remove_dir_all(first);
    let _ = fs::remove_dir_all(second);
}

#[test]
fn source_parent_module_takes_precedence_over_process_cwd() {
    let base = temp_module_dir("source_parent_precedence");
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let module_name = format!("source_parent_policy_{}_{}", std::process::id(), unique);
    let cwd_module = std::env::current_dir()
        .expect("current directory should be readable")
        .join(format!("{module_name}.rl"));
    fs::write(
        &cwd_module,
        r#"
export fun selected_score: () -> Int32 = {
    1
}
"#,
    )
    .expect("cwd shadow module should be written");
    let _cwd_cleanup = RemoveFileOnDrop(cwd_module);

    fs::write(
        base.join(format!("{module_name}.rl")),
        r#"
export fun selected_score: () -> Int32 = {
    42
}
"#,
    )
    .expect("source-parent module should be written");

    let root = parse_complete(&format!(
        r#"
import {module_name}.{{selected_score}}

export fun source_parent_score: () -> Int32 = {{
    () selected_score
}}
"#
    ));
    let resolved = resolve_program_imports_for_file(root, &base.join("app.rl"))
        .expect("source-relative import should resolve");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("source-parent precedence program should type check");
    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("source-parent precedence program should generate WAT");
    let (mut store, instance) = instantiate_wat("source-parent precedence", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "source_parent_score")
        .expect("source-parent precedence export should be host-callable");

    assert_eq!(
        score
            .call(&mut store, ())
            .expect("source-parent precedence export should execute"),
        42,
        "the source file's parent directory must take precedence over process cwd"
    );

    let _ = fs::remove_dir_all(base);
}

#[test]
fn diamond_dependency_is_emitted_once_with_one_identity() {
    let root = parse_complete(
        r#"
import left_policy.{left_score}
import right_policy.{right_score}

export fun diamond_score: () -> Int32 = {
    val left = () left_score;
    val right = () right_score;
    left + right
}
"#,
    );

    let mut sources = HashMap::new();
    sources.insert(
        "shared_score".to_string(),
        r#"
export fun base_score: () -> Int32 = {
    10
}
"#
        .to_string(),
    );
    sources.insert(
        "left_policy".to_string(),
        r#"
import shared_score.{base_score}

export fun left_score: () -> Int32 = {
    val base = () base_score;
    base + 1
}
"#
        .to_string(),
    );
    sources.insert(
        "right_policy".to_string(),
        r#"
import shared_score.{base_score}

export fun right_score: () -> Int32 = {
    val base = () base_score;
    base + 2
}
"#
        .to_string(),
    );

    let resolved = resolve_program_imports_with_module_source_map(root, sources)
        .expect("diamond imports should resolve one shared module identity");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("diamond imports should type check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("diamond imports should generate WAT");
    let internal_base = internal_module_name(&["shared_score"], "base_score");
    assert_eq!(
        wat.matches(&format!("(func ${internal_base}")).count(),
        1,
        "the shared diamond dependency must be emitted once"
    );

    let (mut store, instance) = instantiate_wat("diamond dependency identity", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "diamond_score")
        .expect("diamond regression export should be host-callable");
    assert_eq!(
        score
            .call(&mut store, ())
            .expect("diamond regression export should execute"),
        23
    );
}

#[test]
fn package_root_maps_root_and_submodule_imports_to_source_files() {
    let package_source = temp_module_dir("package_root_and_submodule");
    fs::write(
        package_source.join("lib.rl"),
        r#"
pub fun root_score: () -> Int32 = {
    40
}
"#,
    )
    .expect("package root module should be written");
    fs::write(
        package_source.join("math.rl"),
        r#"
pub fun add_two: (value: Int32) -> Int32 = {
    value + 2
}
"#,
    )
    .expect("package submodule should be written");

    let root = parse_complete(
        r#"
import local_utils.{root_score}
import local_utils.math.{add_two}

pub fun package_root_score: () -> Int32 = {
    val score = () root_score;
    score |> add_two
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("valid package namespace should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("package root and submodule imports should resolve");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("package root and submodule imports should type check");
    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("package root and submodule imports should generate WAT");
    let (mut store, instance) = instantiate_wat("package root and submodule", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "package_root_score")
        .expect("package root regression export should be host-callable");
    assert_eq!(
        score
            .call(&mut store, ())
            .expect("package root regression export should execute"),
        42
    );

    let _ = fs::remove_dir_all(package_source);
}

#[test]
fn package_local_imports_stay_inside_the_registered_namespace() {
    let app_source = temp_module_dir("package_local_app_source");
    let package_source = temp_module_dir("package_local_dependency_source");
    fs::write(
        app_source.join("detail.rl"),
        r#"
pub fun package_bonus: () -> Int32 = {
    99
}
"#,
    )
    .expect("application shadow module should be written");
    fs::write(
        package_source.join("lib.rl"),
        r#"
import detail.{package_bonus}

pub fun package_score: () -> Int32 = {
    () package_bonus
}
"#,
    )
    .expect("package root module should be written");
    fs::write(
        package_source.join("detail.rl"),
        r#"
pub fun package_bonus: () -> Int32 = {
    7
}
"#,
    )
    .expect("package-local detail module should be written");

    let root = parse_complete(
        r#"
import local_utils.{package_score}

pub fun package_local_score: () -> Int32 = {
    () package_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(app_source.clone())
        .expect("application search root should be registered");
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("valid package namespace should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("package-local import should resolve under the package namespace");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("package-local import should type check");
    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("package-local import should generate WAT");
    let canonical_bonus = internal_module_name(&["local_utils", "detail"], "package_bonus");
    assert_eq!(
        wat.matches(&format!("(func ${canonical_bonus}")).count(),
        1,
        "package-local declarations should use their package-qualified canonical identity:\n{wat}"
    );

    let (mut store, instance) = instantiate_wat("package-local import", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "package_local_score")
        .expect("package-local regression export should be host-callable");
    assert_eq!(
        score
            .call(&mut store, ())
            .expect("package-local regression export should execute"),
        7,
        "an unqualified import in a package must not resolve from the application source tree"
    );

    let _ = fs::remove_dir_all(app_source);
    let _ = fs::remove_dir_all(package_source);
}

#[test]
fn configured_package_namespace_does_not_fall_back_to_application_sources() {
    let app_source = temp_module_dir("package_missing_app_source");
    let package_source = temp_module_dir("package_missing_dependency_source");
    fs::create_dir_all(app_source.join("local_utils"))
        .expect("application shadow namespace should be created");
    fs::write(
        app_source.join("local_utils/missing.rl"),
        r#"
pub fun fallback_score: () -> Int32 = {
    99
}
"#,
    )
    .expect("application fallback module should be written");
    fs::write(
        package_source.join("lib.rl"),
        r#"
pub fun available_score: () -> Int32 = {
    1
}
"#,
    )
    .expect("package root module should be written");

    let root = parse_complete(
        r#"
import local_utils.missing.{fallback_score}

fun main: () -> Int32 = {
    () fallback_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(app_source.clone())
        .expect("application search root should be registered");
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("valid package namespace should be registered");
    let error = resolver
        .resolve_program_imports(root)
        .expect_err("missing package module must not fall back to application sources");
    assert!(
        error.to_string().contains("local_utils.missing"),
        "missing package module diagnostic should retain its canonical identity, got: {error}"
    );

    let _ = fs::remove_dir_all(app_source);
    let _ = fs::remove_dir_all(package_source);
}

#[test]
fn package_root_registration_rejects_invalid_and_duplicate_namespaces() {
    let package_source = temp_module_dir("invalid_package_namespaces");

    for namespace in ["bad-name", "fun", "std", "two.parts", "1package"] {
        let mut resolver = ModuleResolver::new();
        let error = resolver
            .add_package_root(namespace.to_string(), package_source.clone())
            .expect_err("invalid or reserved package namespace should be rejected");
        assert!(
            error.to_string().contains(namespace),
            "invalid package namespace diagnostic should name {namespace:?}, got: {error}"
        );
    }

    let other_source = temp_module_dir("duplicate_package_namespace");
    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("first package namespace registration should succeed");
    let error = resolver
        .add_package_root("local_utils".to_string(), other_source.clone())
        .expect_err("duplicate package namespace registration should be rejected");
    assert!(
        error.to_string().contains("local_utils")
            && error.to_string().to_lowercase().contains("duplicate"),
        "duplicate package namespace diagnostic should name the namespace, got: {error}"
    );

    let _ = fs::remove_dir_all(package_source);
    let _ = fs::remove_dir_all(other_source);
}

#[test]
fn separate_package_roots_keep_distinct_deterministic_module_identities() {
    let alpha_source = temp_module_dir("package_identity_alpha");
    let beta_source = temp_module_dir("package_identity_beta");
    for (source, export_name, value) in [
        (&alpha_source, "alpha_score", 20),
        (&beta_source, "beta_score", 22),
    ] {
        fs::write(
            source.join("lib.rl"),
            format!(
                r#"
import detail.{{shared_score}}

pub fun {export_name}: () -> Int32 = {{
    () shared_score
}}
"#
            ),
        )
        .expect("package root module should be written");
        fs::write(
            source.join("detail.rl"),
            format!(
                r#"
pub fun shared_score: () -> Int32 = {{
    {value}
}}
"#
            ),
        )
        .expect("package detail module should be written");
    }

    let root = parse_complete(
        r#"
import alpha_pkg.{alpha_score}
import beta_pkg.{beta_score}

pub fun combined_package_score: () -> Int32 = {
    val alpha = () alpha_score;
    val beta = () beta_score;
    alpha + beta
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("alpha_pkg".to_string(), alpha_source.clone())
        .expect("alpha package namespace should be registered");
    resolver
        .add_package_root("beta_pkg".to_string(), beta_source.clone())
        .expect("beta package namespace should be registered");
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("separate package roots should resolve");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("separate package roots should type check");
    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&resolved)
        .expect("separate package roots should generate WAT");
    for namespace in ["alpha_pkg", "beta_pkg"] {
        let internal_shared = internal_module_name(&[namespace, "detail"], "shared_score");
        assert_eq!(
            wat.matches(&format!("(func ${internal_shared}")).count(),
            1,
            "each package should emit one distinct canonical declaration for shared_score:\n{wat}"
        );
    }

    let (mut store, instance) = instantiate_wat("separate package identities", &wat);
    let score = instance
        .get_typed_func::<(), i32>(&store, "combined_package_score")
        .expect("combined package regression export should be host-callable");
    assert_eq!(
        score
            .call(&mut store, ())
            .expect("combined package regression export should execute"),
        42
    );

    let _ = fs::remove_dir_all(alpha_source);
    let _ = fs::remove_dir_all(beta_source);
}

#[test]
fn package_lib_path_cannot_duplicate_the_namespace_root_identity() {
    let package_source = temp_module_dir("package_lib_identity_alias");
    fs::write(
        package_source.join("lib.rl"),
        r#"
pub record Marker {
    value: Int32
}
"#,
    )
    .expect("package root module should be written");
    let root = parse_complete(
        r#"
import local_utils.lib.{Marker}

fun main: () -> Int32 = {
    1
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("valid package namespace should be registered");
    let error = resolver
        .resolve_program_imports(root)
        .expect_err("alias.lib must not load lib.rl under a second identity");

    assert!(
        error.to_string().contains("aliases the namespace root")
            && error.to_string().contains("local_utils.lib"),
        "root identity diagnostic should explain the alias, got: {error}"
    );
    let _ = fs::remove_dir_all(package_source);
}

#[test]
fn one_package_source_root_cannot_be_registered_under_multiple_aliases() {
    let package_source = temp_module_dir("duplicate_package_source_root");
    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("first_pkg".to_string(), package_source.clone())
        .expect("first package alias should be registered");
    let error = resolver
        .add_package_root("second_pkg".to_string(), package_source.clone())
        .expect_err("one package source root must have one canonical alias");

    assert!(
        error
            .to_string()
            .contains("overlaps the source root registered as namespace 'first_pkg'"),
        "duplicate source-root diagnostic should name the canonical alias, got: {error}"
    );
    let _ = fs::remove_dir_all(package_source);
}

#[cfg(unix)]
#[test]
fn package_module_symlink_cannot_escape_the_registered_source_root() {
    use std::os::unix::fs::symlink;

    let package_source = temp_module_dir("package_symlink_escape_source");
    let outside_source = temp_module_dir("package_symlink_escape_outside");
    let outside_module = outside_source.join("escape.rl");
    fs::write(
        &outside_module,
        r#"
pub fun escaped_score: () -> Int32 = {
    99
}
"#,
    )
    .expect("outside module should be written");
    symlink(&outside_module, package_source.join("escape.rl"))
        .expect("package escape symlink should be created");
    let root = parse_complete(
        r#"
import local_utils.escape.{escaped_score}

fun main: () -> Int32 = {
    () escaped_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("valid package namespace should be registered");
    let error = resolver
        .resolve_program_imports(root)
        .expect_err("package module symlinks must stay below the registered source root");

    assert!(
        error.to_string().contains("escapes source root"),
        "symlink escape diagnostic should identify the boundary, got: {error}"
    );
    let _ = fs::remove_dir_all(package_source);
    let _ = fs::remove_dir_all(outside_source);
}

#[test]
fn package_namespace_registration_is_order_independent_with_virtual_sources() {
    let package_source = temp_module_dir("package_virtual_registration_order");
    fs::write(
        package_source.join("lib.rl"),
        "pub fun package_score: () -> Int32 = { 42 }\n",
    )
    .expect("package root module should be written");

    let mut virtual_first = ModuleResolver::new();
    virtual_first
        .try_add_module_source(
            vec!["local_utils".to_string(), "detail".to_string()],
            "pub fun virtual_score: () -> Int32 = { 99 }".to_string(),
        )
        .expect("initial virtual source should be registered");
    let error = virtual_first
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect_err("a package root must not capture an existing virtual namespace");
    assert!(error
        .to_string()
        .contains("already used by a resolved or virtual source module"));

    let mut package_first = ModuleResolver::new();
    package_first
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("package root should be registered first");
    let error = package_first
        .try_add_module_source(
            vec!["local_utils".to_string(), "detail".to_string()],
            "pub fun virtual_score: () -> Int32 = { 99 }".to_string(),
        )
        .expect_err("a virtual source must not override a package namespace");
    assert!(error
        .to_string()
        .contains("conflicts with its configured package namespace"));

    let _ = fs::remove_dir_all(package_source);
}

#[test]
fn overlapping_package_source_roots_are_rejected() {
    let outer_source = temp_module_dir("overlapping_package_roots");
    let nested_source = outer_source.join("nested");
    fs::create_dir_all(&nested_source).expect("nested package source should be created");
    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("outer_pkg".to_string(), outer_source.clone())
        .expect("outer package root should be registered");
    let error = resolver
        .add_package_root("nested_pkg".to_string(), nested_source)
        .expect_err("overlapping package roots could alias one module file");

    assert!(error.to_string().contains("overlaps the source root"));
    let _ = fs::remove_dir_all(outer_source);
}

#[test]
fn canonical_equivalent_search_roots_are_deduplicated() {
    let search_root = temp_module_dir("canonical_search_root_deduplication");
    let nested = search_root.join("nested");
    fs::create_dir_all(&nested).expect("nested directory should be created");
    fs::write(
        search_root.join("policy.rl"),
        "pub fun score: () -> Int32 = { 42 }\n",
    )
    .expect("module should be written");

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(search_root.clone())
        .expect("first search-root spelling should be registered");
    resolver
        .add_search_path(nested.join(".."))
        .expect("canonical-equivalent search root should be deduplicated");
    resolver
        .resolve_module(&["policy".to_string()])
        .expect("one canonical search root should resolve without ambiguity");

    let _ = fs::remove_dir_all(search_root);
}

#[test]
fn search_and_package_root_containment_is_rejected_in_both_orders() {
    let outer = temp_module_dir("search_package_root_overlap");
    let nested = outer.join("nested");
    fs::create_dir_all(&nested).expect("nested source directory should be created");

    let mut outer_search_first = ModuleResolver::new();
    outer_search_first
        .add_search_path(outer.clone())
        .expect("outer search root should be registered");
    let error = outer_search_first
        .add_package_root("nested_pkg".to_string(), nested.clone())
        .expect_err("a package root nested below a search root must be rejected");
    assert!(
        error
            .to_string()
            .contains("overlaps configured module search root"),
        "search-first diagnostic should identify the overlap, got: {error}"
    );

    let mut nested_package_first = ModuleResolver::new();
    nested_package_first
        .add_package_root("nested_pkg".to_string(), nested.clone())
        .expect("nested package root should be registered");
    let error = nested_package_first
        .add_search_path(outer.clone())
        .expect_err("a search root containing a package root must be rejected");
    assert!(
        error.to_string().contains("overlaps package source root"),
        "package-first diagnostic should identify the overlap, got: {error}"
    );

    let mut nested_search_first = ModuleResolver::new();
    nested_search_first
        .add_search_path(nested.clone())
        .expect("nested search root should be registered");
    let error = nested_search_first
        .add_package_root("outer_pkg".to_string(), outer.clone())
        .expect_err("a package root containing a search root must be rejected");
    assert!(
        error
            .to_string()
            .contains("overlaps configured module search root"),
        "nested search-first diagnostic should identify the overlap, got: {error}"
    );

    let mut outer_package_first = ModuleResolver::new();
    outer_package_first
        .add_package_root("outer_pkg".to_string(), outer.clone())
        .expect("outer package root should be registered");
    let error = outer_package_first
        .add_search_path(nested)
        .expect_err("a search root nested below a package root must be rejected");
    assert!(
        error.to_string().contains("overlaps package source root"),
        "outer package-first diagnostic should identify the overlap, got: {error}"
    );

    let _ = fs::remove_dir_all(outer);
}

#[cfg(unix)]
#[test]
fn one_package_file_cannot_resolve_under_two_module_paths() {
    use std::os::unix::fs::symlink;

    let package_source = temp_module_dir("package_internal_symlink_identity");
    fs::write(
        package_source.join("actual.rl"),
        "pub fun shared_score: () -> Int32 = { 42 }\n",
    )
    .expect("canonical package module should be written");
    symlink(
        package_source.join("actual.rl"),
        package_source.join("alias.rl"),
    )
    .expect("internal package symlink should be created");
    let root = parse_complete(
        r#"
import local_utils.actual.{shared_score}
import local_utils.alias.{shared_score}

fun main: () -> Int32 = {
    1
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("package root should be registered");
    let error = resolver
        .resolve_program_imports(root)
        .expect_err("one physical module file must have one canonical logical identity");

    assert!(
        error.to_string().contains("multiple canonical identities")
            && error.to_string().contains("local_utils.actual")
            && error.to_string().contains("local_utils.alias"),
        "file identity diagnostic should name both module paths, got: {error}"
    );
    let _ = fs::remove_dir_all(package_source);
}

#[cfg(unix)]
#[test]
fn package_owned_files_cannot_be_imported_through_physical_search_paths() {
    use std::os::unix::fs::symlink;

    let app_source = temp_module_dir("package_physical_path_app");
    let package_source = temp_module_dir("package_physical_path_dependency");
    let physical_alias = app_source.join("vendor/local_utils/src");
    fs::create_dir_all(&package_source).expect("vendored package source should be created");
    fs::create_dir_all(&physical_alias).expect("physical alias directory should be created");
    fs::write(
        package_source.join("detail.rl"),
        "pub fun hidden_alias_score: () -> Int32 = { 99 }\n",
    )
    .expect("vendored package module should be written");
    symlink(
        package_source.join("detail.rl"),
        physical_alias.join("detail.rl"),
    )
    .expect("physical module alias should be created");
    let root = parse_complete(
        r#"
import vendor.local_utils.src.detail.{hidden_alias_score}

fun main: () -> Int32 = {
    () hidden_alias_score
}
"#,
    );

    let mut resolver = ModuleResolver::new();
    resolver
        .add_search_path(app_source.clone())
        .expect("application search root should be registered");
    resolver
        .add_package_root("local_utils".to_string(), package_source.clone())
        .expect("vendored package root should be registered");
    let error = resolver
        .resolve_program_imports(root)
        .expect_err("package files must only be reachable through their canonical alias");

    assert!(
        error
            .to_string()
            .contains("inside package namespace 'local_utils'")
            && error.to_string().contains("vendor.local_utils.src.detail"),
        "physical-path diagnostic should require the canonical package alias, got: {error}"
    );
    let _ = fs::remove_dir_all(app_source);
    let _ = fs::remove_dir_all(package_source);
}
