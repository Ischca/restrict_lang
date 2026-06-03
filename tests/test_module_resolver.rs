use restrict_lang::ast::{ImportItems, TopDecl, Type};
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
    resolver.add_search_path(dir.clone());
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
    resolver.add_search_path(dir.clone());
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("record and function imports should resolve");

    assert!(resolved.imports.is_empty());
    assert!(matches!(
        resolved.declarations.first(),
        Some(TopDecl::Record(record)) if record.name == "ReleaseSlice"
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
    resolver.add_search_path(dir.clone());
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
    resolver.add_search_path(dir.clone());
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
    assert!(wat.contains("(func $__rl_mod_modules_release_scores_score"));
    assert!(wat.contains("(func $score"));
    assert!(wat.contains("call $__rl_mod_modules_release_scores_score"));
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
    resolver.add_search_path(dir.clone());
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("imports should resolve without leaking private helper names");

    let internal_name = "__rl_mod_release_score";
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
    assert!(wat.contains("(func $__rl_mod_release_score"));
    assert!(wat.contains("(func $score"));
    assert!(wat.contains("call $__rl_mod_release_score"));
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
    resolver.add_search_path(dir.clone());
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
    assert!(wat.contains("(func $__rl_mod_score_util_score"));
    assert!(wat.contains("(func $score"));
    assert!(wat.contains("call $__rl_mod_score_util_score"));
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
    resolver.add_search_path(dir.clone());
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("private context dependency should resolve");

    assert!(
        resolved.declarations.iter().any(|decl| {
            matches!(decl, TopDecl::Context(context) if context.name == "__rl_mod_release_Policy")
        }),
        "private context should be emitted under an internal module name"
    );
    assert!(
        resolved.declarations.iter().any(|decl| {
            matches!(decl, TopDecl::Record(record) if record.name == "__rl_mod_release_PolicyLimits")
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
                            Type::Named(name) if name == "__rl_mod_release_PolicyLimits"
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
    assert!(wat.contains(";; With context: __rl_mod_release_Policy"));
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
    resolver.add_search_path(dir.clone());
    let resolved = resolver
        .resolve_program_imports(root)
        .expect("private impl dependency should resolve");

    assert!(
        resolved.declarations.iter().any(|decl| {
            matches!(decl, TopDecl::Record(record) if record.name == "__rl_mod_release_Signal")
        }),
        "private method receiver record should be emitted under an internal module name"
    );
    assert!(
        resolved
            .declarations
            .iter()
            .filter(|decl| {
                matches!(decl, TopDecl::Impl(impl_block) if impl_block.target == "__rl_mod_release_Signal")
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
    assert!(wat.contains("(func $__rl_mod_release_Signal_risk_score"));
    assert!(wat.contains("(func $__rl_mod_release_Signal_risk_bucket"));
    assert!(wat.contains("call $__rl_mod_release_Signal_risk_score"));
    assert!(wat.contains("call $__rl_mod_release_Signal_risk_bucket"));
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
    resolver.add_search_path(dir.clone());
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
    resolver.add_search_path(dir.clone());
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
    resolver.add_search_path(dir.clone());
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
