use restrict_lang::ast::{
    BlockExpr, Expr, FieldDecl, FieldInit, FunDecl, Program, PrototypeCloneExpr, RecordDecl,
    RecordLit, TopDecl, Type,
};
use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_to_wat(source: &str) -> Result<String, String> {
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

fn parse_complete(source: &str) -> Program {
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely, remaining: {remaining:?}"
    );
    program
}

fn int32() -> Type {
    Type::Named("Int32".to_string())
}

fn record_decl(name: &str, fields: Vec<(&str, Type)>) -> TopDecl {
    TopDecl::Record(RecordDecl {
        name: name.to_string(),
        type_params: Vec::new(),
        temporal_constraints: Vec::new(),
        fields: fields
            .into_iter()
            .map(|(name, ty)| FieldDecl {
                name: name.to_string(),
                ty,
            })
            .collect(),
        frozen: false,
        sealed: false,
        parent_hash: None,
    })
}

#[test]
fn exported_generic_function_rejects_with_explicit_codegen_error() {
    let source = r#"
export fun keep: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    1 |> keep
}
"#;

    let err = compile_to_wat(source).expect_err("exported generic function should need an ABI");
    assert!(
        err.contains("Codegen error: Unsupported feature: Exported generic function 'keep'"),
        "error should explain exported generic ABI limitation, got: {err}"
    );
    assert!(
        err.contains("requires a concrete ABI"),
        "error should identify the missing concrete ABI, got: {err}"
    );
}

#[test]
fn unsupported_export_kind_lists_current_codegen_export_surface() {
    let source = r#"
export context ReleasePolicy {
    score: Int32
}

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("context exports should not have a Wasm ABI");
    assert!(
        err.contains("Only concrete function exports, source-level record exports, and constant global exports are supported by codegen"),
        "error should list the current export support surface, got: {err}"
    );
}

#[test]
fn unresolved_source_import_rejects_before_codegen_panics() {
    let program = parse_complete(
        r#"
import release.{public_score}

fun main: () -> Int32 = {
    1
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("unresolved imports should be rejected by codegen");
    let message = err.to_string();

    assert!(
        message.contains(
            "Unsupported feature: source-level imports must be resolved before code generation"
        ),
        "error should explain that imports need resolver expansion, got: {message}"
    );
    assert!(
        message.contains("release.{public_score}"),
        "error should identify the unresolved import, got: {message}"
    );
}

#[test]
fn mutable_top_level_global_rejects_with_explicit_codegen_error() {
    let source = r#"
mut val counter = 0

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("mutable globals should be outside v0.0.1 codegen");
    assert!(
        err.contains("Codegen error: Unsupported feature: Top-level mutable bindings are not supported by codegen yet"),
        "error should explain mutable top-level global limitation, got: {err}"
    );
}

#[test]
fn runtime_allocated_top_level_global_rejects_with_explicit_codegen_error() {
    let source = r#"
record ReleaseSlice {
    score: Int32
}

val slice = ReleaseSlice { score: 42 }

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("record global requires runtime allocation");
    assert!(
        err.contains("Codegen error: Unsupported feature: Top-level binding of type"),
        "error should explain unsupported runtime global initialization, got: {err}"
    );
    assert!(
        err.contains("requires runtime initialization"),
        "error should identify the runtime allocation boundary, got: {err}"
    );
}

#[test]
fn prototype_identity_clone_rejects_with_explicit_codegen_error() {
    let program = Program {
        imports: Vec::new(),
        declarations: vec![
            record_decl("Base", vec![("id", int32())]),
            TopDecl::Function(FunDecl {
                name: "main".to_string(),
                is_async: false,
                type_params: Vec::new(),
                temporal_constraints: Vec::new(),
                params: Vec::new(),
                return_type: Some(Type::Named("Base".to_string())),
                body: BlockExpr {
                    statements: Vec::new(),
                    expr: Some(Box::new(Expr::PrototypeClone(PrototypeCloneExpr {
                        base: "Base".to_string(),
                        updates: RecordLit {
                            name: "Base".to_string(),
                            fields: vec![FieldInit::Field {
                                name: "id".to_string(),
                                value: Box::new(Expr::IntLit(7)),
                            }],
                        },
                        freeze_immediately: false,
                        sealed: false,
                    }))),
                },
            }),
        ],
    };

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("prototype clone must fail instead of emitting placeholder hashes");
    let message = err.to_string();

    assert!(
        message.contains("Unsupported feature: prototype clone for 'Base'"),
        "error should identify unsupported prototype clone codegen, got: {message}"
    );
    assert!(
        message.contains("real prototype identity metadata"),
        "error should explain why placeholder metadata is rejected, got: {message}"
    );
}

#[test]
fn ambiguous_impl_method_rejects_with_explicit_codegen_error() {
    let program = parse_complete(
        r#"
record ReleaseScore {
    value: Int32
}

record BuildScore {
    value: Int32
}

impl ReleaseScore {
    fun label: (self: ReleaseScore) -> Int32 = {
        self.value
    }
}

impl BuildScore {
    fun label: (self: BuildScore) -> Int32 = {
        self.value
    }
}

fun main: () -> Int32 = {
    val unknown = 1;
    (unknown) label
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("ambiguous impl methods need an explicit codegen diagnostic");
    let message = err.to_string();

    assert!(
        message.contains("Unsupported feature: Restrict OSV method call 'label' is ambiguous"),
        "error should identify ambiguous Restrict method resolution, got: {message}"
    );
    assert!(
        message.contains("receiver type could not be inferred"),
        "error should explain the missing receiver type, got: {message}"
    );
    assert!(
        !message.contains("Feature not implemented"),
        "ambiguous method dispatch should be an explicit release boundary, got: {message}"
    );
}

#[test]
fn function_return_annotation_supplies_lambda_codegen_abi() {
    let source = r#"
fun make_adjuster: () -> Float64 -> Float64 = {
    |value| value + 0.5
}

fun main: () -> Float64 = {
    val adjust = () make_adjuster;
    2.0 |> adjust
}
"#;

    let wat = compile_to_wat(source)
        .expect("function return annotation should provide the lambda runtime ABI");
    assert!(
        wat.contains("closure_call_1_f64_to_f64"),
        "lambda returned from a function should use the annotated Float64 ABI:\n{wat}"
    );
    assert!(
        wat.contains("(param $value f64)"),
        "lambda parameter should not fall back to Int32 codegen:\n{wat}"
    );
}

#[test]
fn function_value_call_result_supports_record_field_access_codegen() {
    let source = r#"
record Box {
    value: Int32
}

fun make_box: () -> Box = {
    Box { value: 7 }
}

fun main: () -> Int32 = {
    val maker: () -> Box = make_box;
    (() maker).value
}
"#;

    let wat = compile_to_wat(source)
        .expect("grouped OSV function value call result should support field access");

    assert!(
        wat.contains("call_indirect"),
        "function value call should lower through the closure ABI:\n{wat}"
    );
    assert!(
        wat.contains("i32.load"),
        "field access on the function value result should load the record field:\n{wat}"
    );
}

#[test]
fn function_typed_record_field_pipe_target_generates_closure_call() {
    let source = r#"
record Strategy {
    mapper: Int32 -> Int32
}

export fun field_mapper_score: () -> Int32 = {
    val strategy = Strategy {
        mapper: |score| score + 1
    };
    41 |> (strategy.mapper)
}
"#;

    let wat = compile_to_wat(source)
        .expect("function-typed record fields should lower as callable pipe targets");

    assert!(
        wat.contains("call_indirect"),
        "function-typed record field call should lower through the closure ABI:\n{wat}"
    );
    assert!(
        wat.contains("call_indirect (type $closure_call_1)"),
        "function-typed record field call should use the one-argument Int32 closure ABI:\n{wat}"
    );
}

#[test]
fn untyped_lambda_return_rejects_instead_of_int32_fallback() {
    let program = parse_complete(
        r#"
fun make_mapper: () = {
    |value| value
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not invent an Int32 return ABI for an untyped lambda");
    let message = err.to_string();

    assert!(
        message.contains(
            "function 'make_mapper' return ABI requires a return type annotation or inferable body source type"
        ),
        "error should explain the missing function return ABI, got: {message}"
    );
}

#[test]
fn uninferable_function_return_abi_rejects_instead_of_int32_fallback() {
    let program = parse_complete(
        r#"
fun empty_values: () = {
    []
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not invent an Int32 return ABI");
    let message = err.to_string();

    assert!(
        message.contains(
            "function 'empty_values' return ABI requires a return type annotation or inferable body source type"
        ),
        "error should explain the missing function return ABI, got: {message}"
    );
}

#[test]
fn unknown_named_type_rejects_instead_of_pointer_abi_fallback() {
    let program = parse_complete(
        r#"
fun score_missing: (value: MissingType) -> Int32 = {
    1
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not treat an unknown named type as a pointer ABI");
    let message = err.to_string();

    assert!(
        message.contains("unknown source type 'MissingType' has no Wasm ABI"),
        "error should identify the unknown source type ABI, got: {message}"
    );
}

#[test]
fn unknown_generic_constructor_rejects_instead_of_pointer_abi_fallback() {
    let program = parse_complete(
        r#"
fun await_future: (value: Future<Int32>) -> Int32 = {
    1
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not treat an unknown generic constructor as a pointer ABI");
    let message = err.to_string();

    assert!(
        message.contains("generic source type 'Future<Int32>' has no Wasm ABI"),
        "error should identify the unknown generic source ABI, got: {message}"
    );
}

#[test]
fn unsupported_local_type_annotation_reports_function_codegen_context() {
    let program = parse_complete(
        r#"
fun main: () -> Int32 = {
    val missing: MissingType = 1;
    1
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not invent a local ABI for an unknown annotation");
    let message = err.to_string();

    assert!(
        message
            .contains("unknown source type 'MissingType' has no Wasm ABI while generating 'main'"),
        "error should identify the unsupported local source ABI and function, got: {message}"
    );
}

#[test]
fn unsupported_local_generic_annotation_reports_function_codegen_context() {
    let program = parse_complete(
        r#"
fun main: () -> Int32 = {
    val pending: Future<Int32> = 1;
    1
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not invent a local ABI for an unknown generic annotation");
    let message = err.to_string();

    assert!(
        message.contains(
            "generic source type 'Future<Int32>' has no Wasm ABI while generating 'main'"
        ),
        "error should identify the unsupported local generic ABI and function, got: {message}"
    );
}

#[test]
fn exported_record_parameter_rejects_composite_host_abi() {
    let source = r#"
record ReleaseSlice {
    score: Int32
}

export fun public_score: (slice: ReleaseSlice) -> Int32 = {
    slice.score
}

fun main: () -> Int32 = {
    val slice = ReleaseSlice { score: 41 };
    slice |> public_score
}
"#;

    let err = compile_to_wat(source).expect_err("record parameters need a designed host ABI");
    assert!(
        err.contains("Exported function 'public_score' parameter 'slice' type ReleaseSlice requires a composite host ABI"),
        "error should reject composite export parameters explicitly, got: {err}"
    );
}

#[test]
fn exported_record_return_rejects_composite_host_abi() {
    let source = r#"
record ReleaseSlice {
    score: Int32
}

export fun build_slice: () -> ReleaseSlice = {
    ReleaseSlice { score: 41 }
}

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("record returns need a designed host ABI");
    assert!(
        err.contains(
            "Exported function 'build_slice' return type ReleaseSlice requires a composite host ABI"
        ),
        "error should reject composite export returns explicitly, got: {err}"
    );
}

#[test]
fn exported_list_parameter_rejects_generic_composite_host_abi() {
    let source = r#"
export fun public_total: (scores: List<Int32>) -> Int32 = {
    1
}

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("List parameters need a designed host ABI");
    assert!(
        err.contains(
            "Exported function 'public_total' parameter 'scores' type List<Int32> requires a composite host ABI"
        ),
        "error should reject generic composite export parameters explicitly, got: {err}"
    );
}

#[test]
fn exported_option_return_rejects_generic_composite_host_abi() {
    let source = r#"
export fun public_score: () -> Option<Int32> = {
    Some(1)
}

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("Option returns need a designed host ABI");
    assert!(
        err.contains(
            "Exported function 'public_score' return type Option<Int32> requires a composite host ABI"
        ),
        "error should reject generic composite export returns explicitly, got: {err}"
    );
}

#[test]
fn inferred_scalar_export_return_is_allowed() {
    let source = r#"
export fun public_score: (value: Int32) = {
    value + 1
}

fun main: () -> Int32 = {
    1
}
"#;

    let wat = compile_to_wat(source).expect("inferred scalar export return should be allowed");
    assert!(
        wat.contains("(export \"public_score\" (func $public_score))"),
        "scalar inferred export should be emitted as a host export:\n{wat}"
    );
}

#[test]
fn inferred_string_export_return_rejects_composite_host_abi() {
    let source = r#"
export fun release_label: () = {
    "stable"
}

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("inferred String exports need a designed ABI");
    assert!(
        err.contains(
            "Exported function 'release_label' return type String requires a composite host ABI"
        ),
        "error should reject inferred String export returns explicitly, got: {err}"
    );
}

#[test]
fn inferred_list_export_return_rejects_composite_host_abi() {
    let source = r#"
export fun release_scores: () = {
    [1, 2, 3]
}

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("inferred List exports need a designed ABI");
    assert!(
        err.contains(
            "Exported function 'release_scores' return type List<Int32> requires a composite host ABI"
        ),
        "error should reject inferred List export returns explicitly, got: {err}"
    );
}

#[test]
fn main_is_emitted_as_start_with_current_function_syntax() {
    let source = r#"
fun main: () -> Int32 = {
    42
}
"#;

    let wat = compile_to_wat(source).expect("main should generate WAT");
    assert!(
        wat.contains("(func $main (result i32)"),
        "source-level main should keep its declared result ABI:\n{wat}"
    );
    assert!(
        wat.contains("(func $__restrict_start")
            && wat.contains("call $main\n    drop")
            && wat.contains("(export \"_start\" (func $__restrict_start))"),
        "main should be reached through a no-result host _start wrapper:\n{wat}"
    );
}

#[test]
fn parameterized_main_keeps_function_abi_without_start_wrapper() {
    let source = r#"
fun main: (flag: Boolean) -> Int32 = {
    flag then {
        1
    } else {
        0
    }
}
"#;

    let wat = compile_to_wat(source).expect("parameterized main should generate WAT");
    assert!(
        wat.contains("(func $main (param $flag i32) (result i32)"),
        "parameterized main should keep its declared function ABI:\n{wat}"
    );
    assert!(
        !wat.contains("$__restrict_start") && !wat.contains("(export \"_start\""),
        "only zero-argument main should emit a host _start wrapper:\n{wat}"
    );
}

#[test]
fn v001_record_field_access_does_not_surface_not_implemented_diagnostic() {
    let source = r#"
record ReleaseScore {
    value: Int32
}

fun main: () -> Int32 = {
    val score = ReleaseScore { value: 41 };
    score.value + 1
}
"#;

    let wat = match compile_to_wat(source) {
        Ok(wat) => wat,
        Err(err) => {
            assert!(
                !err.contains("Feature not implemented"),
                "v0.0.1 record field access should not surface CodeGenError::NotImplemented in user-facing diagnostics, got: {err}"
            );
            panic!("v0.0.1 record field access should compile, got: {err}");
        }
    };

    assert!(
        wat.contains("i32.load"),
        "record field access should load the Int32 field from linear memory:\n{wat}"
    );
}

#[test]
fn exported_string_global_rejects_composite_host_abi() {
    let source = r#"
export val release_label: String = "stable"

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("String global exports need a designed host ABI");
    assert!(
        err.contains("Exported top-level binding 'release_label' has type String which requires a composite host ABI"),
        "error should reject composite global exports explicitly, got: {err}"
    );
}

#[test]
fn exported_list_global_rejects_composite_host_abi_before_runtime_initialization() {
    let source = r#"
export val release_scores: List<Int32> = [1, 2]

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("List global exports need a designed host ABI");
    assert!(
        err.contains("Exported top-level binding 'release_scores' has type List<Int32> which requires a composite host ABI"),
        "error should reject composite global exports explicitly, got: {err}"
    );
}

#[test]
fn exported_option_global_rejects_composite_host_abi_before_runtime_initialization() {
    let source = r#"
export val release_owner: Option<Int32> = Some(7)

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("Option global exports need a designed host ABI");
    assert!(
        err.contains("Exported top-level binding 'release_owner' has type Option<Int32> which requires a composite host ABI"),
        "error should reject composite global exports explicitly, got: {err}"
    );
}

#[test]
fn exported_result_global_rejects_composite_host_abi_before_runtime_initialization() {
    let source = r#"
export val release_route: Result<Int32, Int32> = Ok(42)

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("Result global exports need a designed host ABI");
    assert!(
        err.contains("Exported top-level binding 'release_route' has type Result<Int32, Int32> which requires a composite host ABI"),
        "error should reject composite global exports explicitly, got: {err}"
    );
}

#[test]
fn exported_record_global_rejects_composite_host_abi_before_runtime_initialization() {
    let source = r#"
record ReleaseScore {
    value: Int32
}

export val release_score: ReleaseScore = ReleaseScore { value: 42 }

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("record global exports need a designed host ABI");
    assert!(
        err.contains("Exported top-level binding 'release_score' has type ReleaseScore which requires a composite host ABI"),
        "error should reject composite global exports explicitly, got: {err}"
    );
}

#[test]
fn unknown_record_literal_layout_rejects_instead_of_offset_fallback() {
    let program = parse_complete(
        r#"
fun main: () -> Int32 = {
    val slice = ReleaseSlice { score: 41 };
    1
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not synthesize unknown record field offsets");
    let message = err.to_string();

    assert!(
        message.contains("invalid record layout")
            && message.contains("field 'score' in record 'ReleaseSlice'"),
        "error should identify the unknown record field layout, got: {message}"
    );
    assert!(
        !message.contains("Feature not implemented"),
        "record layout failures should be explicit codegen boundaries, got: {message}"
    );
}

#[test]
fn clone_non_record_base_rejects_with_explicit_codegen_error() {
    let program = parse_complete(
        r#"
fun main: () -> Int32 = {
    val base = 1;
    val updated = base.clone { value: 2 };
    0
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("clone on a scalar base should reject explicitly");
    let message = err.to_string();

    assert!(
        message.contains("record clone requires a record base") && message.contains("Int32"),
        "error should explain the non-record clone base, got: {message}"
    );
    assert!(
        !message.contains("Feature not implemented"),
        "clone base failures should be explicit codegen boundaries, got: {message}"
    );
}

#[test]
fn fold_accumulator_codegen_rejects_uninferable_initial_value() {
    let program = parse_complete(
        r#"
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    (numbers, [], |total, value| total) fold
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("codegen should not invent an Int32 fold accumulator");
    let message = err.to_string();

    assert!(
        message.contains("fold accumulator requires an inferable source type"),
        "error should explain the missing accumulator source type, got: {message}"
    );
}

#[test]
fn println_rejects_unsupported_argument_type_instead_of_string_fallback() {
    let program = parse_complete(
        r#"
fun main: () -> () = {
    3.14 |> println
}
"#,
    );

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("println should not default unsupported values to the String ABI");
    let message = err.to_string();

    assert!(
        message.contains("println does not support argument type Float64"),
        "error should identify the unsupported println argument type, got: {message}"
    );
}
