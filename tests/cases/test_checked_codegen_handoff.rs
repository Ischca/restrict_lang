use restrict_lang::ir::builder::{build_checked_ir, CheckedProgramIr};
use restrict_lang::ir::{ScalarRepr, ValueRepr};
use restrict_lang::module::resolve_program_imports_with_module_source_map;
use restrict_lang::{parse_program, CodeGenError, Program, TypeChecker, WasmCodeGen};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

fn checked_program(source: &str) -> (Program, CheckedProgramIr) {
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely: {remaining:?}"
    );

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("source should type check");
    let checked_ir = build_checked_ir(&program, &checker).expect("checked IR should build");
    (program, checked_ir)
}

fn generate_checked(source: &str) -> String {
    let (program, checked_ir) = checked_program(source);
    WasmCodeGen::new()
        .generate_checked(&program, &checked_ir)
        .expect("checked codegen should succeed")
}

fn assert_valid_wat(wat: &str) {
    let wasm = wat::parse_str(wat).expect("generated WAT should assemble");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("generated Wasm should validate");
}

fn assert_invalid_checked_ir(error: CodeGenError) {
    assert!(
        matches!(error, CodeGenError::InvalidCheckedIr(_)),
        "expected checked IR error, got: {error}"
    );
}

#[test]
fn checked_and_legacy_codegen_match_for_annotated_scalar_program() {
    let source = r#"
fun add_one: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> add_one
}
"#;
    let (program, checked_ir) = checked_program(source);
    let legacy = WasmCodeGen::new()
        .generate(&program)
        .expect("legacy codegen should succeed");
    let checked = WasmCodeGen::new()
        .generate_checked(&program, &checked_ir)
        .expect("checked codegen should succeed");

    assert_eq!(checked, legacy);
    assert_valid_wat(&checked);
}

#[test]
fn checked_codegen_uses_inferred_forward_float64_return_abi() {
    let wat = generate_checked(
        r#"
fun adjusted: (value: Float64) = {
    value |> risk
}

fun risk: (value: Float64) = {
    value + 0.5
}

fun main: () -> Float64 = {
    41.5 |> adjusted
}
"#,
    );

    assert!(
        wat.contains("(func $adjusted (param $value f64) (result f64)"),
        "checked return ABI should be f64:\n{wat}"
    );
    assert_valid_wat(&wat);
}

#[test]
fn checked_codegen_specializes_unannotated_generic_return_from_checked_signature() {
    let wat = generate_checked(
        r#"
fun id_local: <T>(value: T) = {
    value
}

fun main: () -> Float64 = {
    1.5 |> id_local
}
"#,
    );

    let specialization = wat
        .lines()
        .find(|line| line.contains("(func $id_local__Float64$sid$"))
        .expect("generic specialization should carry its injective identity suffix");
    assert!(
        specialization.contains("(param $value f64) (result f64)"),
        "generic specialization should use the checked Float64 result:\n{wat}"
    );
    assert_valid_wat(&wat);
}

#[test]
fn checked_codegen_preserves_first_class_container_builtin_calls() {
    let wat = generate_checked(
        r#"
fun main: () -> Option<String> = {
    val maybe: Option<Int32> = (7) Option::Some;
    val apply_map = map;
    (maybe, |value| "ok") apply_map
}
"#,
    );

    assert_valid_wat(&wat);
}

#[test]
fn checked_codegen_supports_inferred_result_constructor_locals() {
    let wat = generate_checked(include_str!(
        "../../examples/dogfood_result_local_inference.rl"
    ));

    assert_valid_wat(&wat);
}

#[test]
fn checked_codegen_supports_release_readiness_inference() {
    let wat = generate_checked(include_str!(
        "../../examples/dogfood_release_readiness_inference.rl"
    ));

    assert_valid_wat(&wat);
}

#[test]
fn checked_codegen_supports_generic_constructor_context_inference() {
    let wat = generate_checked(include_str!(
        "../../examples/dogfood_generic_context_inference.rl"
    ));

    assert_valid_wat(&wat);
}

#[test]
fn checked_codegen_preserves_current_unit_parameter_and_result_abi() {
    let wat = generate_checked(
        r#"
fun consume: (value: ()) -> () = {
    ()
}

fun main: () -> () = {
    ()
}
"#,
    );

    let consume_header = wat
        .lines()
        .find(|line| line.contains("(func $consume"))
        .expect("consume function should be emitted");
    assert!(consume_header.contains("(param $value i32)"));
    assert!(!consume_header.contains("(result"));
    assert_valid_wat(&wat);
}

#[test]
fn checked_codegen_rejects_missing_duplicate_and_stale_function_facts() {
    let source = r#"
fun identity: (value: Int32) -> Int32 = {
    value
}
"#;
    let (program, checked_ir) = checked_program(source);

    let mut missing = checked_ir.clone();
    missing.functions.clear();
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&program, &missing)
            .expect_err("missing checked function should fail"),
    );

    let mut duplicate = checked_ir.clone();
    duplicate.functions.push(duplicate.functions[0].clone());
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&program, &duplicate)
            .expect_err("duplicate checked function should fail"),
    );

    let mut extra = checked_ir.clone();
    let mut unexpected = extra.functions[0].clone();
    unexpected.name = "unexpected".to_string();
    extra.functions.push(unexpected);
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&program, &extra)
            .expect_err("extra checked function should fail"),
    );

    let mut renamed_param = checked_ir.clone();
    renamed_param.functions[0].params[0].name = "other".to_string();
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&program, &renamed_param)
            .expect_err("renamed checked parameter should fail"),
    );

    let mut stale_export = checked_ir.clone();
    stale_export.functions[0].lowering.source_exported = true;
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&program, &stale_export)
            .expect_err("stale checked export status should fail"),
    );

    let (_, stale_float_ir) = checked_program(
        r#"
fun identity: (value: Float64) -> Float64 = {
    value
}
"#,
    );
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&program, &stale_float_ir)
            .expect_err("stale checked signature should fail"),
    );

    let (inferred_int_program, _) = checked_program(
        r#"
fun inferred: () = {
    1
}
"#,
    );
    let (_, inferred_float_ir) = checked_program(
        r#"
fun inferred: () = {
    1.5
}
"#,
    );
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&inferred_int_program, &inferred_float_ir)
            .expect_err("checked facts from a different inferred body should fail"),
    );

    let (_, mut transplanted_ir) = checked_program(
        r#"
fun inferred: () = {
    1
}
"#,
    );
    transplanted_ir.functions = inferred_float_ir.functions.clone();
    transplanted_ir.layout_table = inferred_float_ir.layout_table.clone();
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&inferred_int_program, &transplanted_ir)
            .expect_err("transplanted checked facts should fail their construction seal"),
    );
}

#[test]
fn checked_codegen_rejects_stale_repr_and_lowering_summary_before_emission() {
    let source = r#"
fun identity: (value: Int32) -> Int32 = {
    value
}
"#;
    let (program, checked_ir) = checked_program(source);

    let mut stale_repr = checked_ir.clone();
    stale_repr.functions[0].return_repr = ValueRepr::Scalar(ScalarRepr::F64);
    let mut codegen = WasmCodeGen::new();
    assert_invalid_checked_ir(
        codegen
            .generate_checked(&program, &stale_repr)
            .expect_err("stale representation should fail"),
    );

    let wat = codegen
        .generate_checked(&program, &checked_ir)
        .expect("failed validation must not partially mutate the generator");
    assert_valid_wat(&wat);

    let mut stale_summary = checked_ir.clone();
    stale_summary.functions[0].lowering.param_host_abis.clear();
    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&program, &stale_summary)
            .expect_err("stale lowering summary should fail"),
    );
}

#[test]
fn checked_codegen_rejects_changed_node_identity_but_accepts_clones() {
    let source = r#"
fun identity: (value: Int32) -> Int32 = {
    value
}
"#;
    let (program, checked_ir) = checked_program(source);

    WasmCodeGen::new()
        .generate_checked(&program.clone(), &checked_ir)
        .expect("an equal AST clone should retain checked identity");

    let mut changed_identity = program.clone();
    let restrict_lang::ast::TopDecl::Function(function) = &mut changed_identity.declarations[0]
    else {
        panic!("test source should contain a function");
    };
    function
        .body
        .expr
        .as_mut()
        .expect("identity body should have a result expression")
        .id = restrict_lang::ast::NodeId::DUMMY;

    assert_invalid_checked_ir(
        WasmCodeGen::new()
            .generate_checked(&changed_identity, &checked_ir)
            .expect_err("changed source node identity should invalidate checked facts"),
    );
}

#[test]
fn cli_compiles_through_checked_forward_float64_handoff() {
    let nonce = std::process::id();
    let source_path = std::env::temp_dir().join(format!("restrict_checked_handoff_{nonce}.rl"));
    let output_path = std::env::temp_dir().join(format!("restrict_checked_handoff_{nonce}.wat"));
    let source = r#"
fun adjusted: (value: Float64) = {
    value |> risk
}

fun risk: (value: Float64) -> Float64 = {
    value + 0.5
}

export fun score: () -> Float64 = {
    41.5 |> adjusted
}
"#;
    fs::write(&source_path, source).expect("CLI source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg(&source_path)
        .arg(&output_path)
        .output()
        .expect("compiler CLI should run");
    assert!(
        output.status.success(),
        "checked CLI compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let wat = fs::read_to_string(&output_path).expect("CLI should write WAT");
    assert!(wat.contains("(func $adjusted (param $value f64) (result f64)"));
    assert_valid_wat(&wat);

    let _ = fs::remove_file(source_path);
    let _ = fs::remove_file(output_path);
}

#[test]
fn checked_codegen_uses_post_resolution_node_identity() {
    let root = r#"
import release.{public_score}

fun main: () -> Int32 = {
    41 |> public_score
}
"#;
    let (remaining, root_program) = parse_program(root).expect("root source should parse");
    assert!(remaining.trim().is_empty());

    let mut modules = HashMap::new();
    modules.insert(
        "release".to_string(),
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}
"#
        .to_string(),
    );
    let resolved = resolve_program_imports_with_module_source_map(root_program, modules)
        .expect("module should resolve and renumber the combined AST");

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved)
        .expect("resolved source should type check");
    let checked_ir =
        build_checked_ir(&resolved, &checker).expect("resolved checked IR should build");
    let resolved_clone = resolved.clone();
    let wat = WasmCodeGen::new()
        .generate_checked(&resolved_clone, &checked_ir)
        .expect("an equal clone of the resolved AST should keep checked facts valid");

    assert!(wat.contains("call $public_score"));
    assert_valid_wat(&wat);
}
