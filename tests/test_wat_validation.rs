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

fn parse_source(source: &str) -> restrict_lang::ast::Program {
    let (remaining, ast) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely, remaining: {remaining:?}"
    );
    ast
}

fn assert_valid_wat(name: &str, source: &str) -> String {
    let wat = compile_to_wat(source).unwrap_or_else(|err| {
        panic!("{name} should compile before WAT validation: {err}");
    });

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("{name} generated invalid WAT: {err}\n\n{wat}");
    });

    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("{name} generated invalid Wasm binary: {err}\n\n{wat}");
        });

    wat
}

#[test]
fn context_bindings_generate_valid_wat() {
    assert_valid_wat(
        "context_bindings",
        r#"
context Scale {
    factor: Float64
}

fun main: () -> Float64 = {
    val input = 2.5;
    with Scale { factor: input } {
        factor + 0.5
    }
}
"#,
    );
}

#[test]
fn branch_local_names_with_different_wasm_types_generate_valid_wat() {
    let wat = assert_valid_wat(
        "branch_local_names_with_different_wasm_types",
        r#"
fun main: () -> Int32 = {
    true then {
        val scratch = 1.5;
        0
    } else {
        val scratch = 1;
        scratch
    }
}
"#,
    );

    assert!(
        wat.contains("__local_"),
        "conflicting branch-local source names should be emitted with a Wasm alias:\n{wat}"
    );
}

#[test]
fn impl_methods_generate_valid_wat_with_type_directed_dispatch() {
    assert_valid_wat(
        "impl_methods_type_directed_dispatch",
        r#"
record Score {
    value: Int32
}

record Penalty {
    value: Int32
}

impl Score {
    fun amount: (self: Score) -> Int32 = {
        self.value
    }
}

impl Penalty {
    fun amount: (self: Penalty) -> Int32 = {
        0 - self.value
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 11 };
    (score) amount
}
"#,
    );
}

#[test]
fn impl_method_dispatch_on_record_returning_expression_generates_valid_wat() {
    assert_valid_wat(
        "impl_method_dispatch_on_record_returning_expression",
        r#"
record Score {
    value: Int32
}

impl Score {
    fun total: (self: Score) -> Int32 = {
        self.value
    }
}

fun make_score: () -> Score = {
    Score { value: 41 }
}

fun main: () -> Int32 = {
    (() make_score) total
}
"#,
    );
}

#[test]
fn duplicate_impl_method_dispatch_on_record_returning_expression_generates_valid_wat() {
    assert_valid_wat(
        "duplicate_impl_method_dispatch_on_record_returning_expression",
        r#"
record Score {
    value: Int32
}

record Penalty {
    value: Int32
}

impl Score {
    fun amount: (self: Score) -> Int32 = {
        self.value
    }
}

impl Penalty {
    fun amount: (self: Penalty) -> Int32 = {
        0 - self.value
    }
}

fun make_score: () -> Score = {
    Score { value: 41 }
}

fun main: () -> Int32 = {
    (() make_score) amount
}
"#,
    );
}

#[test]
fn unannotated_impl_method_float_return_generates_valid_wat() {
    assert_valid_wat(
        "unannotated_impl_method_float_return",
        r#"
record Score {
    value: Float64
}

impl Score {
    fun risk: (self: Score) = {
        self.value + 0.5
    }
}

fun read_risk: (score: Score) -> Float64 = {
    (score) risk
}

fun main: () -> Float64 = {
    val score = Score { value: 41.5 };
    score |> read_risk
}
"#,
    );
}

#[test]
fn unannotated_impl_method_calling_later_annotated_method_generates_valid_wat() {
    assert_valid_wat(
        "unannotated_impl_method_calling_later_annotated_method",
        r#"
record Score {
    value: Float64
}

impl Score {
    fun adjusted: (self: Score) = {
        (self) risk
    }

    fun risk: (self: Score) -> Float64 = {
        self.value + 0.5
    }
}

fun main: () -> Float64 = {
    val score = Score { value: 41.5 };
    (score) adjusted
}
"#,
    );
}

#[test]
fn unannotated_float_function_generates_valid_wat() {
    assert_valid_wat(
        "unannotated_float_function",
        r#"
fun adjust: (value: Float64) = {
    value + 0.5
}

fun main: () -> Float64 = {
    41.5 |> adjust
}
"#,
    );
}

#[test]
fn unannotated_function_calling_later_annotated_float_generates_valid_wat() {
    assert_valid_wat(
        "unannotated_function_calling_later_annotated_float",
        r#"
fun adjust: (value: Float64) = {
    value |> risk
}

fun risk: (value: Float64) -> Float64 = {
    value + 0.5
}

fun main: () -> Float64 = {
    41.5 |> adjust
}
"#,
    );
}

#[test]
fn practical_examples_generate_valid_wat() {
    assert_valid_wat(
        "checkout_review",
        include_str!("../examples/checkout_review.rl"),
    );
    assert_valid_wat(
        "order_pricing",
        include_str!("../examples/order_pricing.rl"),
    );
    assert_valid_wat(
        "generic_inference",
        include_str!("../examples/generic_inference.rl"),
    );
    assert_valid_wat("deploy_gate", include_str!("../examples/deploy_gate.rl"));
    assert_valid_wat(
        "incident_triage",
        include_str!("../examples/incident_triage.rl"),
    );
    assert_valid_wat(
        "fulfillment_batch",
        include_str!("../examples/fulfillment_batch.rl"),
    );
    assert_valid_wat(
        "inventory_reorder",
        include_str!("../examples/inventory_reorder.rl"),
    );
    assert_valid_wat(
        "sensor_calibration",
        include_str!("../examples/sensor_calibration.rl"),
    );
    assert_valid_wat(
        "status_routing",
        include_str!("../examples/status_routing.rl"),
    );
    assert_valid_wat(
        "result_validation",
        include_str!("../examples/result_validation.rl"),
    );
    assert_valid_wat("retry_budget", include_str!("../examples/retry_budget.rl"));
    assert_valid_wat(
        "release_readiness",
        include_str!("../examples/release_readiness.rl"),
    );
    assert_valid_wat(
        "sprint_planner",
        include_str!("../examples/sprint_planner.rl"),
    );
    assert_valid_wat(
        "support_queue",
        include_str!("../examples/support_queue.rl"),
    );
    assert_valid_wat(
        "service_monitor",
        include_str!("../examples/service_monitor.rl"),
    );
    assert_valid_wat(
        "rollout_handoff",
        include_str!("../examples/rollout_handoff.rl"),
    );
    assert_valid_wat(
        "feature_rollout_policy",
        include_str!("../examples/feature_rollout_policy.rl"),
    );
    assert_valid_wat(
        "bug_triage_board",
        include_str!("../examples/bug_triage_board.rl"),
    );
    assert_valid_wat(
        "review_policy_factory",
        include_str!("../examples/review_policy_factory.rl"),
    );
    assert_valid_wat(
        "change_review_gate",
        include_str!("../examples/change_review_gate.rl"),
    );
    assert_valid_wat(
        "calibration_pipeline",
        include_str!("../examples/calibration_pipeline.rl"),
    );
    assert_valid_wat(
        "experiment_scorecard",
        include_str!("../examples/experiment_scorecard.rl"),
    );
    assert_valid_wat(
        "release_queue_snapshot",
        include_str!("../examples/release_queue_snapshot.rl"),
    );
    assert_valid_wat(
        "release_review_digest",
        include_str!("../examples/release_review_digest.rl"),
    );
    assert_valid_wat(
        "generic_function_value_pipeline",
        include_str!("../examples/generic_function_value_pipeline.rl"),
    );
    assert_valid_wat(
        "release_decision_engine",
        include_str!("../examples/release_decision_engine.rl"),
    );
    assert_valid_wat(
        "typed_impl_dispatch",
        include_str!("../examples/typed_impl_dispatch.rl"),
    );
}

#[test]
fn source_imports_are_rejected_before_type_checking() {
    let source = r#"
import release.{public_score}

fun main: () -> Int32 = {
    1
}
"#;

    let err = compile_to_wat(source).expect_err("unresolved import should be rejected");

    assert!(
        err.contains("source-level imports must be resolved before type checking"),
        "error should explain that imports are not wired into type checking yet, got: {err}"
    );
    assert!(
        err.contains("release.{public_score}"),
        "error should name the unresolved import, got: {err}"
    );
}

#[test]
fn codegen_rejects_unresolved_imports_instead_of_emitting_fake_abi() {
    let source = r#"
import release.*

fun main: () -> Int32 = {
    1
}
"#;
    let ast = parse_source(source);
    let mut codegen = WasmCodeGen::new();

    let err = codegen
        .generate(&ast)
        .expect_err("codegen should reject unresolved source imports");
    let message = err.to_string();

    assert!(
        message.contains("source-level imports must be resolved before code generation"),
        "error should explain that source imports must be resolved, got: {message}"
    );
    assert!(
        message.contains("release.*"),
        "error should name the unresolved wildcard import, got: {message}"
    );
}

#[test]
fn user_generic_direct_float_call_generates_valid_wat() {
    let source = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Float64 = {
    1.5 |> id_local
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("generic float call should compile: {err}"));
    assert!(
        wat.contains("$id_local__Float64"),
        "Float64 generic call should use a specialized f64 ABI function:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic float call generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic float call generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn unannotated_user_generic_float_call_generates_valid_wat() {
    let source = r#"
fun id_local: <T>(value: T) = {
    value
}

fun main: () -> Float64 = {
    1.5 |> id_local
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("unannotated generic float call should compile: {err}"));
    assert!(
        wat.contains("$id_local__Float64"),
        "unannotated Float64 generic call should specialize from the inferred return type:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("unannotated generic float call generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("unannotated generic float call generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn user_generic_function_value_mapper_generates_valid_wat() {
    let source = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    (numbers, id_local) map
}
"#;

    assert_valid_wat("user_generic_function_value_mapper", source);
}

#[test]
fn unannotated_user_generic_function_value_mapper_generates_valid_wat() {
    let source = r#"
fun id_local: <T>(value: T) = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    (numbers, id_local) map
}
"#;

    assert_valid_wat("unannotated_user_generic_function_value_mapper", source);
}

#[test]
fn unannotated_user_generic_option_mapper_generates_valid_wat() {
    let source = r#"
fun wrap: <T>(value: T) = {
    Some(value)
}

fun main: () -> List<Option<Float64>> = {
    val readings = [1.5, 2.5];
    (readings, wrap) map
}
"#;

    assert_valid_wat("unannotated_user_generic_option_mapper", source);
}

#[test]
fn user_generic_float_function_value_mapper_generates_valid_wat() {
    let source = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Float64> = {
    val readings = [1.5, 2.5, 3.5];
    (readings, id_local) map
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("generic float mapper should compile: {err}"));
    assert!(
        wat.contains("$id_local__Float64"),
        "Float64 generic mapper should use a specialized f64 ABI function:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic float mapper generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic float mapper generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn user_generic_empty_list_uses_expected_return_for_codegen() {
    let source = r#"
fun empty_list: <T>() -> List<T> = {
    []
}

fun main: () -> List<Float64> = {
    () empty_list
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("generic empty list should compile: {err}"));
    assert!(
        wat.contains("$empty_list__Float64"),
        "empty generic list should specialize from the expected return type:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic empty list generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic empty list generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn user_generic_empty_option_uses_binding_annotation_for_codegen() {
    let source = r#"
fun empty_option: <T>() -> Option<T> = {
    None
}

fun main: () -> Option<Int32> = {
    val missing: Option<Int32> = () empty_option;
    missing
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("generic empty option should compile: {err}"));
    assert!(
        wat.contains("$empty_option__Int32"),
        "empty generic option should specialize from the binding annotation:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic empty option generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic empty option generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn exported_function_generates_definition_and_valid_wat() {
    let source = r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> public_score
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("exported function should compile: {err}"));
    assert!(
        wat.contains("(func $public_score"),
        "exported function should still generate a function body:\n{wat}"
    );
    assert!(
        wat.contains("(export \"public_score\" (func $public_score))"),
        "exported function should emit a matching Wasm export:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("exported function generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("exported function generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn pub_function_generates_definition_and_valid_wat() {
    let source = r#"
pub fun public_score: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> public_score
}
"#;

    let wat =
        compile_to_wat(source).unwrap_or_else(|err| panic!("pub function should compile: {err}"));
    assert!(
        wat.contains("(func $public_score"),
        "pub function should still generate a function body:\n{wat}"
    );
    assert!(
        wat.contains("(export \"public_score\" (func $public_score))"),
        "pub function should emit a matching Wasm export:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("pub function generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("pub function generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn exported_string_return_rejects_composite_host_abi() {
    let source = r#"
export fun release_label: () -> String = {
    "stable"
}

fun main: () -> String = {
    () release_label
}
"#;

    let err = compile_to_wat(source).expect_err("String exports need a designed host ABI");
    assert!(
        err.contains(
            "Exported function 'release_label' return type String requires a composite host ABI"
        ),
        "error should reject composite export returns explicitly, got: {err}"
    );
}

#[test]
fn top_level_primitive_constants_generate_globals() {
    let source = r#"
val release_bias: Int32 = 3
val large_budget: Int64 = 9000000000
val confidence_floor: Float64 = 1.5
val feature_enabled: Boolean = true
val delimiter: Char = ':'
val shipping_label: String = "stable"
val no_payload: () = ()

fun score: (base: Int32) -> Int32 = {
    base + release_bias
}

fun label: () -> String = {
    shipping_label
}

fun main: () -> Int32 = {
    39 |> score
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("top-level constants should compile: {err}"));
    assert!(
        wat.contains("(global $release_bias i32 (i32.const 3))"),
        "Int32 top-level constant should generate an immutable global:\n{wat}"
    );
    assert!(
        wat.contains("(global $large_budget i64 (i64.const 9000000000))"),
        "Int64 top-level constant should generate an immutable global:\n{wat}"
    );
    assert!(
        wat.contains("(global $confidence_floor f64 (f64.const 1.5))"),
        "Float64 top-level constant should generate an immutable global:\n{wat}"
    );
    assert!(
        wat.contains("(global $feature_enabled i32 (i32.const 1))"),
        "Boolean top-level constant should generate an immutable global:\n{wat}"
    );
    assert!(
        wat.contains("(global $delimiter i32 (i32.const 58))"),
        "Char top-level constant should generate an immutable global:\n{wat}"
    );
    assert!(
        wat.contains("(global $no_payload i32 (i32.const 0))"),
        "Unit top-level constant should generate an immutable global:\n{wat}"
    );
    assert!(
        wat.contains("global.get $release_bias"),
        "function body should read the top-level constant via global.get:\n{wat}"
    );
    assert!(
        wat.contains("global.get $shipping_label"),
        "String top-level constant should be available to functions:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("top-level constants generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("top-level constants generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn exported_top_level_constant_generates_global_export() {
    let source = r#"
export val release_bias: Int32 = 3

fun main: () -> Int32 = {
    release_bias + 39
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("exported top-level constant should compile: {err}"));
    assert!(
        wat.contains("(global $release_bias i32 (i32.const 3))"),
        "exported top-level constant should generate a global:\n{wat}"
    );
    assert!(
        wat.contains("(export \"release_bias\" (global $release_bias))"),
        "exported top-level constant should emit a Wasm global export:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("exported top-level constant generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("exported top-level constant generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn exported_scalar_top_level_constants_generate_global_exports() {
    let source = r#"
export val release_bias: Int32 = 3
export val large_budget: Int64 = 9000000000
export val confidence_floor: Float64 = 1.5
export val feature_enabled: Boolean = true
export val delimiter: Char = ':'
export val no_payload: () = ()

fun main: () -> Int32 = {
    release_bias + 39
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("exported scalar constants should compile: {err}"));
    for expected in [
        "(export \"release_bias\" (global $release_bias))",
        "(export \"large_budget\" (global $large_budget))",
        "(export \"confidence_floor\" (global $confidence_floor))",
        "(export \"feature_enabled\" (global $feature_enabled))",
        "(export \"delimiter\" (global $delimiter))",
        "(export \"no_payload\" (global $no_payload))",
    ] {
        assert!(
            wat.contains(expected),
            "scalar top-level constant should emit Wasm global export `{expected}`:\n{wat}"
        );
    }

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("exported scalar constants generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("exported scalar constants generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn pub_top_level_constant_generates_global_export() {
    let source = r#"
pub val release_bias: Int32 = 3

fun main: () -> Int32 = {
    release_bias + 39
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("pub top-level constant should compile: {err}"));
    assert!(
        wat.contains("(global $release_bias i32 (i32.const 3))"),
        "pub top-level constant should generate a global:\n{wat}"
    );
    assert!(
        wat.contains("(export \"release_bias\" (global $release_bias))"),
        "pub top-level constant should emit a Wasm global export:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("pub top-level constant generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("pub top-level constant generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn pub_record_is_source_level_only_and_emits_no_wasm_export() {
    let source = r#"
pub record ReleaseSlice {
    score: Int32
}

fun main: () -> Int32 = {
    1
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("pub record should compile as source-level export: {err}"));
    assert!(
        wat.contains("source export record ReleaseSlice has no direct Wasm export"),
        "WAT should document source-level record export:\n{wat}"
    );
    assert!(
        !wat.contains("(export \"ReleaseSlice\""),
        "record export must not imply a host-visible Wasm ABI:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("pub record generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("pub record generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn runtime_allocated_top_level_bindings_are_rejected() {
    assert_runtime_top_level_binding_rejected(
        "record",
        r#"
record ReleaseSlice {
    score: Int32
}

val slice = ReleaseSlice { score: 42 }

fun main: () -> Int32 = {
    1
}
"#,
    );

    assert_runtime_top_level_binding_rejected(
        "list",
        r#"
val scores: List<Int32> = [1, 2, 3]

fun main: () -> Int32 = {
    1
}
"#,
    );

    assert_runtime_top_level_binding_rejected(
        "option",
        r#"
val maybe_score: Option<Int32> = Some(42)

fun main: () -> Int32 = {
    1
}
"#,
    );

    assert_runtime_top_level_binding_rejected(
        "result",
        r#"
val route: Result<Int32, String> = Ok(42)

fun main: () -> Int32 = {
    1
}
"#,
    );
}

fn assert_runtime_top_level_binding_rejected(label: &str, source: &str) {
    let err = compile_to_wat(source).unwrap_err();
    assert!(
        err.contains("Top-level binding of type"),
        "error should explain unsupported runtime global initialization for {label}, got: {err}"
    );
}

#[test]
fn exported_generic_function_is_rejected_before_invalid_wat() {
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
        err.contains("Exported generic function"),
        "error should explain exported generic ABI limitation, got: {err}"
    );
}

#[test]
fn nested_record_field_patterns_use_distinct_parent_scratch_locals() {
    let source = r#"
record OwnerSignal {
    confidence: Option<Int32>,
    queue: List<Int32>
}

record ReleaseSlice {
    owner: OwnerSignal,
    blocker_codes: List<Int32>,
    fallback: Int32
}

fun summarize_slice: (slice: ReleaseSlice) -> Int32 = {
    val ReleaseSlice {
        owner: OwnerSignal {
            confidence: Some(confidence),
            queue: [first_queue, second_queue]
        },
        fallback
    } = slice;

    confidence + first_queue + second_queue + fallback
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("nested record field patterns should compile: {err}"));
    assert!(
        wat.contains("local.set $record_tmp_0"),
        "outer record pattern should preserve its parent in a pattern scratch local:\n{wat}"
    );
    assert!(
        wat.contains("local.set $record_tmp_1"),
        "nested record pattern should use a distinct scratch local:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("nested record field patterns generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("nested record field patterns generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn std_list_update_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val appended = ([1, 2], 3) list_append;
    val prepended = (0, appended) list_prepend;
    val combined = (prepended, [4, 5]) list_concat;
    combined |> list_count
}
"#;

    assert_valid_wat("std_list_update_functions", source);
}

#[test]
fn std_math_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val a = -5 |> abs;
    val b = (10, 20) max;
    val c = (3, 7) min;
    val d = (2, 3) pow;
    val e = 4 |> factorial;

    a + b + c + d + e
}
"#;

    assert_valid_wat("std_math_functions", source);
}

#[test]
fn float_math_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val a = -3.14 |> abs_f;
    val b = (1.5, 2.7) max_f;
    val c = (0.5, 1.0) min_f;

    a + b + c
}
"#;

    assert_valid_wat("float_math_functions", source);
}

#[test]
fn std_prelude_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> Boolean = {
    val bool_not = false |> not;
    val bool_and = (bool_not, true) and;
    val bool_or = (bool_and, false) or;
    (bool_or, "prelude boolean flow") assert;

    bool_or
}
"#;

    assert_valid_wat("std_prelude_functions", source);
}

#[test]
fn std_io_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> () = {
    "Hello" |> print;
    42 |> print_int;
    3.14 |> print_float;
    "Error" |> eprint;
    "Error with newline" |> eprintln
}
"#;

    assert_valid_wat("std_io_functions", source);
}

#[test]
fn unannotated_named_function_value_locals_generate_valid_wat() {
    assert_valid_wat(
        "unannotated_named_function_value_local_int32",
        r#"
fun score: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    val mapper = score;
    41 |> mapper
}
"#,
    );

    assert_valid_wat(
        "unannotated_named_function_value_local_float64",
        r#"
fun score: (value: Float64) -> Float64 = {
    value + 0.5
}

fun main: () -> Float64 = {
    val mapper = score;
    41.5 |> mapper
}
"#,
    );
}

#[test]
fn unit_returning_named_function_values_generate_valid_wat() {
    let source = r#"
fun bind_sidecar: (code: Int32) -> () = {
    ()
}

fun call_sidecar: (code: Int32) -> () = {
    ()
}

fun run_sidecar: (sidecar: Int32 -> (), code: Int32) -> () = {
    code |> sidecar
}

fun main: () -> () = {
    val expected_sidecar: Int32 -> () = bind_sidecar;
    7 |> expected_sidecar;
    (call_sidecar, 11) run_sidecar
}
"#;

    assert_valid_wat("unit_returning_named_function_values", source);
}

#[test]
fn std_list_access_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    mut val numbers = [1, 2, 3];
    val empty = numbers |> list_is_empty;
    val first = numbers |> list_head;
    val rest = numbers |> list_tail;
    val reversed = numbers |> list_reverse;
    val first_value = first match {
        Some(value) => { value }
        None => { 0 }
    };
    val rest_count = rest match {
        Some(items) => { items |> list_count }
        None => { 0 }
    };
    val reversed_first = (reversed, 0) list_get;

    empty then {
        0
    } else {
        first_value + rest_count + reversed_first
    }
}
"#;

    assert_valid_wat("std_list_access_functions", source);
}

#[test]
fn float_list_update_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val appended = ([1.5, 2.5], 3.5) list_append;
    val prepended = (0.5, appended) list_prepend;
    val combined = (prepended, [4.5]) list_concat;
    (combined, 0) list_get
}
"#;

    assert_valid_wat("float_list_update_functions", source);
}

#[test]
fn float_std_list_access_functions_use_f64_helpers() {
    let source = r#"
fun main: () -> Float64 = {
    mut val readings = [1.5, 2.5, 3.5];
    val maybe_head = readings |> list_head;
    val maybe_tail = readings |> list_tail;
    val reversed = readings |> list_reverse;
    val head_value = maybe_head match {
        Some(value) => { value }
        None => { 0.0 }
    };
    val tail_count = maybe_tail match {
        Some(rest) => { rest |> list_count }
        None => { 0 }
    };
    val first_reversed = (reversed, 0) list_get;

    tail_count > 0 then {
        head_value + first_reversed
    } else {
        head_value
    }
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("float std list access should compile: {err}"));
    assert!(
        wat.contains("call $list_head_f64"),
        "Float64 list_head should use the f64 ABI helper:\n{wat}"
    );
    assert!(
        wat.contains("call $list_tail_f64"),
        "Float64 list_tail should use the f64 ABI helper:\n{wat}"
    );
    assert!(
        wat.contains("call $list_reverse_f64"),
        "Float64 list_reverse should use the f64 ABI helper:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("float std list access generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("float std list access generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn tail_alias_uses_float64_helper() {
    let source = r#"
fun main: () -> Float64 = {
    val readings: List<Float64> = [1.5, 2.5, 3.5];
    val rest = readings |> tail;
    (rest, 0) list_get
}
"#;

    let wat = assert_valid_wat("tail_alias_float64", source);
    assert!(
        wat.contains("call $tail_f64"),
        "Float64 tail should use the f64 ABI helper:\n{wat}"
    );
    assert!(
        wat.contains("call $list_get_f64"),
        "Float64 tail result should keep the f64 List ABI:\n{wat}"
    );
}

#[test]
fn std_option_functions_generate_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    mut val some: Option<Int32> = Some(42);
    mut val none: Option<Int32> = None;
    val has_value = some |> option_is_some;
    val is_missing = none |> option_is_none;
    val value = (some, 0) option_unwrap_or;
    val fallback = (none, 7) option_unwrap_or;

    (has_value && is_missing) then {
        value + fallback
    } else {
        0
    }
}
"#;

    assert_valid_wat("std_option_functions", source);
}

#[test]
fn float_option_unwrap_or_uses_f64_helper() {
    let source = r#"
fun main: () -> Float64 = {
    mut val some: Option<Float64> = Some(1.5);
    mut val none: Option<Float64> = None;
    val value = (some, 0.0) option_unwrap_or;
    val fallback = (none, 2.5) option_unwrap_or;

    value + fallback
}
"#;

    let wat = compile_to_wat(source)
        .unwrap_or_else(|err| panic!("float option_unwrap_or should compile: {err}"));
    assert!(
        wat.contains("call $option_unwrap_or_f64"),
        "Float64 option_unwrap_or should use the f64 ABI helper:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("float option_unwrap_or generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("float option_unwrap_or generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn float_prelude_iteration_generates_valid_wat() {
    assert_valid_wat(
        "float_map_to_float",
        r#"
fun main: () -> List<Float64> = {
    val numbers = [1, 2];
    (numbers, |value| 1.5) map
}
"#,
    );

    assert_valid_wat(
        "float_list_filter",
        r#"
fun main: () -> List<Float64> = {
    val readings = [1.5, 2.5];
    (readings, |value| value > 2.0) filter
}
"#,
    );

    assert_valid_wat(
        "float_filter_with_capture",
        r#"
fun main: () -> List<Float64> = {
    val threshold = 2.0;
    val readings = [1.5, 2.5];
    (readings, |value| value > threshold) filter
}
"#,
    );

    assert_valid_wat(
        "float_option_map",
        r#"
fun main: () -> Option<Float64> = {
    val reading: Option<Float64> = Some(1.5);
    (reading, |value| value + 1.0) map
}
"#,
    );

    assert_valid_wat(
        "float_option_filter",
        r#"
fun main: () -> Option<Float64> = {
    val reading: Option<Float64> = Some(1.5);
    (reading, |value| value > 1.0) filter
}
"#,
    );

    assert_valid_wat(
        "float_fold_accumulator",
        r#"
fun main: () -> Float64 = {
    val numbers = [1, 2];
    (numbers, 0.0, |total, value| total + 1.0) fold
}
"#,
    );
}

#[test]
fn named_function_iterators_generate_valid_wat() {
    let source = r#"
fun score: (value: Int32) -> Float64 = {
    value > 0 then {
        1.5
    } else {
        0.5
    }
}

fun keep_large: (value: Float64) -> Boolean = {
    value > 1.0
}

fun add_score: (total: Float64, value: Float64) -> Float64 = {
    total + value
}

fun main: () -> Float64 = {
    val numbers = [1, 2, 3];
    val scored = (numbers, score) map;
    val kept = (scored, keep_large) filter;
    (kept, 0.0, add_score) fold
}
"#;

    assert_valid_wat("named_function_iterators", source);
}

#[test]
fn typed_function_value_calls_generate_valid_wat() {
    assert_valid_wat(
        "float_function_value_call",
        r#"
fun main: () -> Float64 = {
    val scale: Float64 -> Float64 = |x| x + 1.5;
    val result = 2.0 |> scale;
    result
}
"#,
    );

    assert_valid_wat(
        "float_named_function_value_call",
        r#"
fun adjust: (value: Float64) -> Float64 = {
    value + 1.5
}

fun main: () -> Float64 = {
    val scale: Float64 -> Float64 = adjust;
    val result = 2.0 |> scale;
    result
}
"#,
    );

    assert_valid_wat(
        "identity_float_function_value_call",
        r#"
fun main: () -> Float64 = {
    val keep: Float64 -> Float64 = identity;
    1.5 |> keep
}
"#,
    );
}

#[test]
fn block_local_float_result_generates_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val result = {
        val x = 1.5;
        x
    };
    result
}
"#;

    assert_valid_wat("block_local_float_result", source);
}

#[test]
fn match_arm_local_float_result_generates_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val result = true match {
        true => {
            val x = 1.5;
            x
        }
        false => {
            val y = 2.5;
            y
        }
    };
    result
}
"#;

    assert_valid_wat("match_arm_local_float_result", source);
}

#[test]
fn direct_float_closure_capture_generates_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val offset = 1.5;
    val add: Float64 -> Float64 = |value| value + offset;
    2.0 |> add
}
"#;

    let wat = assert_valid_wat("direct_float_closure_capture", source);
    assert!(
        wat.contains("f64.store"),
        "Float64 closure capture should store the captured value with f64 ABI:\n{wat}"
    );
    assert!(
        wat.contains("f64.load"),
        "Float64 closure capture should load the captured value with f64 ABI:\n{wat}"
    );
}

#[test]
fn direct_int64_closure_capture_generates_valid_wat() {
    let source = r#"
fun main: () -> Int64 = {
    val offset: Int64 = 10000000000;
    val add: Int64 -> Int64 = |value| value + offset;
    2 |> add
}
"#;

    let wat = assert_valid_wat("direct_int64_closure_capture", source);
    assert!(
        wat.contains("i64.store"),
        "Int64 closure capture should store the captured value with i64 ABI:\n{wat}"
    );
    assert!(
        wat.contains("i64.load"),
        "Int64 closure capture should load the captured value with i64 ABI:\n{wat}"
    );
}

#[test]
fn direct_int64_named_call_uses_parameter_abi_for_small_literals() {
    let source = r#"
fun keep64: (value: Int64) -> Int64 = {
    value
}

fun main: () -> Int64 = {
    (2) keep64
}
"#;

    let wat = assert_valid_wat("direct_int64_named_call", source);
    assert!(
        wat.contains("i64.const 2"),
        "direct Int64 call should emit the small literal with the parameter ABI:\n{wat}"
    );
}

#[test]
fn direct_int64_function_value_call_uses_parameter_abi_for_small_literals() {
    let source = r#"
fun keep64: (value: Int64) -> Int64 = {
    value
}

fun main: () -> Int64 = {
    val keep: Int64 -> Int64 = keep64;
    (2) keep
}
"#;

    let wat = assert_valid_wat("direct_int64_function_value_call", source);
    assert!(
        wat.contains("i64.const 2"),
        "direct Int64 function value call should emit the small literal with the callable ABI:\n{wat}"
    );
}

#[test]
fn int64_identity_uses_expected_abi_for_small_literals() {
    assert_valid_wat(
        "int64_identity_pipe",
        r#"
fun main: () -> Int64 = {
    2 |> identity
}
"#,
    );

    let wat = assert_valid_wat(
        "int64_identity_call",
        r#"
fun main: () -> Int64 = {
    (2) identity
}
"#,
    );
    assert!(
        wat.contains("i64.const 2"),
        "identity under Int64 expected type should emit the small literal with i64 ABI:\n{wat}"
    );
}

#[test]
fn prelude_identity_generates_valid_wat() {
    assert_valid_wat(
        "identity_direct_int_pipe",
        r#"
fun main: () -> Int32 = {
    42 |> identity
}
"#,
    );

    assert_valid_wat(
        "identity_direct_float_pipe",
        r#"
fun main: () -> Float64 = {
    1.5 |> identity
}
"#,
    );

    assert_valid_wat(
        "identity_int_mapper",
        r#"
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    (numbers, identity) map
}
"#,
    );

    assert_valid_wat(
        "identity_float_mapper",
        r#"
fun main: () -> List<Float64> = {
    val readings = [1.5, 2.5];
    (readings, identity) map
}
"#,
    );
}

#[test]
fn binding_patterns_generate_valid_wat() {
    assert_valid_wat(
        "option_binding_pattern",
        r#"
fun main: () -> Int32 = {
    val Some(value) = Some(42);
    value
}
"#,
    );

    assert_valid_wat(
        "result_float_binding_pattern",
        r#"
fun main: () -> Float64 = {
    val Ok(reading): Result<Float64, Int32> = Ok(1.5);
    reading
}
"#,
    );

    assert_valid_wat(
        "record_literal_binding_pattern",
        r#"
record Reading {
    celsius: Float64,
    stable: Boolean
}

fun main: () -> Float64 = {
    val Reading { celsius, stable: true } = Reading {
        celsius: 21.5,
        stable: true
    };
    celsius
}
"#,
    );

    assert_valid_wat(
        "float_list_cons_binding_pattern",
        r#"
fun main: () -> Float64 = {
    val [head | tail] = [1.5, 2.5];
    val [next | _] = tail;
    head + next
}
"#,
    );

    assert_valid_wat(
        "float_list_exact_binding_pattern",
        r#"
fun main: () -> Float64 = {
    val [first, second] = [1.5, 2.5];
    first + second
}
"#,
    );
}

#[test]
fn float_list_cons_binding_uses_f64_tail() {
    let source = r#"
fun main: () -> Float64 = {
    val [head | tail] = [1.5, 2.5];
    val [next | _] = tail;
    head + next
}
"#;

    let wat = compile_to_wat(source).expect("Float64 cons binding should compile to WAT");
    assert!(
        wat.contains("call $tail_f64"),
        "Float64 list cons binding should preserve 8-byte tail layout:\n{}",
        wat
    );

    let wasm = wat::parse_str(&wat).expect("Float64 cons binding should generate valid WAT");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("Float64 cons binding should generate valid Wasm");
}

#[test]
fn float_conditionals_and_arithmetic_generate_valid_wat() {
    let source = r#"
fun choose_scale: (enabled: Boolean) -> Float64 = {
    enabled then {
        1.5 + 2.0
    } else {
        4.0 / 2.0
    }
}

fun compare_scale: (value: Float64) -> Int32 = {
    value > 2.0 then {
        1
    } else {
        0
    }
}

fun main: () -> Float64 = {
    val selected: Float64 = true |> choose_scale;
    val _rank = selected |> compare_scale;
    selected
}
"#;

    assert_valid_wat("float_conditionals", source);
}

#[test]
fn float_modulo_generates_valid_wat() {
    let source = r#"
fun wrap_phase: (value: Float64, period: Float64) -> Float64 = {
    value % period
}

fun main: () -> Float64 = {
    (7.5, 2.0) wrap_phase
}
"#;

    assert_valid_wat("float_modulo", source);
}

#[test]
fn float_record_fields_generate_valid_wat() {
    let source = r#"
record Reading {
    celsius: Float64,
    threshold: Float64
}

fun main: () -> Float64 = {
    val reading = Reading { celsius: 21.5, threshold: 20.0 };
    reading.celsius + 1.0
}
"#;

    assert_valid_wat("float_record_fields", source);
}

#[test]
fn float_record_destructuring_generates_valid_wat() {
    let source = r#"
record Reading {
    celsius: Float64,
    threshold: Float64
}

fun spread: (reading: Reading) -> Float64 = {
    val Reading { celsius, threshold } = reading;
    celsius > threshold then {
        celsius - threshold
    } else {
        threshold - celsius
    }
}

fun main: () -> Float64 = {
    val reading = Reading { celsius: 21.5, threshold: 20.0 };
    reading |> spread
}
"#;

    assert_valid_wat("float_record_destructuring", source);
}

#[test]
fn record_spread_update_generates_valid_wat() {
    let source = r#"
record Profile {
    id: Int32,
    display_name: String,
    score: Float64,
    active: Boolean
}

fun rename: (profile: Profile) -> Profile = {
    Profile {
        ...profile,
        display_name: "Ada Lovelace",
        score: 99.5
    }
}

fun main: () -> Float64 = {
    val base = Profile {
        id: 1,
        display_name: "Ada",
        score: 97.0,
        active: true
    };
    val updated = base |> rename;
    updated.score
}
"#;

    assert_valid_wat("record_spread_update", source);
}

#[test]
fn record_rest_binding_generates_valid_wat() {
    let source = r#"
record Incident {
    id: Int32,
    status: String,
    severity: Int32,
    open: Boolean
}

fun score: (incident: Incident) -> Int32 = {
    val Incident { status, ...rest } = incident;
    val status_score = status match {
        "page" => {
            10
        }
        _ => {
            0
        }
    };
    val active = rest.open then {
        1
    } else {
        0
    };
    status_score + rest.severity + active
}

fun main: () -> Int32 = {
    val incident = Incident {
        id: 7,
        status: "page",
        severity: 5,
        open: true
    };
    incident |> score
}
"#;

    assert_valid_wat("record_rest_binding", source);
}

#[test]
fn record_rest_match_generates_valid_wat() {
    let source = r#"
record Route {
    status: String,
    load: Float64,
    open: Boolean
}

fun score: (route: Route) -> Int32 = {
    route match {
        Route { status: "page", ...rest } => {
            rest.open then {
                10
            } else {
                1
            }
        }
        Route { status, load, open } => {
            load > 0.5 then {
                2
            } else {
                0
            }
        }
    }
}

fun main: () -> Int32 = {
    val route = Route {
        status: "stable",
        load: 0.75,
        open: true
    };
    route |> score
}
"#;

    assert_valid_wat("record_rest_match", source);
}

#[test]
fn nested_record_spread_match_generates_valid_wat() {
    let source = r#"
record Contact {
    email: String,
    verified: Boolean,
    pager: Int32
}

record User {
    id: Int32,
    role: String,
    contact: Contact,
    active: Boolean
}

fun route: (user: User) -> Int32 = {
    user match {
        User { role: "admin", contact: Contact { email, verified: true, pager }, ..._ } => {
            pager
        }
        User { contact: Contact { email: "ops", verified, ..._ }, ..._ } => {
            verified then {
                2
            } else {
                1
            }
        }
        _ => {
            0
        }
    }
}

fun main: () -> Int32 = {
    val contact = Contact {
        email: "ops",
        verified: true,
        pager: 42
    };
    val user = User {
        id: 7,
        role: "admin",
        contact: contact,
        active: true
    };
    user |> route
}
"#;

    assert_valid_wat("nested_record_spread_match", source);
}

#[test]
fn field_access_on_record_returning_expression_generates_valid_wat() {
    let source = r#"
record Envelope {
    value: Int32,
    ok: Boolean
}

fun pass: (item: Envelope) -> Envelope = {
    item
}

fun main: () -> Int32 = {
    val item = Envelope { value: 7, ok: true };
    val value = (item |> pass).value;
    value
}
"#;

    assert_valid_wat("field_access_on_record_returning_expression", source);
}

#[test]
fn inferred_record_return_through_local_binding_generates_valid_wat() {
    let source = r#"
record Envelope {
    value: Int32,
    ok: Boolean
}

fun keep: (item: Envelope) = {
    val saved = item;
    saved
}

fun main: () -> Int32 = {
    val item = Envelope { value: 7, ok: true };
    (item |> keep).value
}
"#;

    assert_valid_wat("inferred_record_return_through_local_binding", source);
}

#[test]
fn inferred_record_return_from_match_binding_generates_valid_wat() {
    let source = r#"
record Envelope {
    value: Int32,
    ok: Boolean
}

fun choose: (maybe: Option<Envelope>, fallback: Envelope) = {
    maybe match {
        Some(item) => {
            item
        }
        None => {
            fallback
        }
    }
}

fun main: () -> Int32 = {
    val item = Envelope { value: 9, ok: true };
    val fallback = Envelope { value: 1, ok: false };
    val chosen = (Some(item), fallback) choose;
    chosen.value
}
"#;

    assert_valid_wat("inferred_record_return_from_match_binding", source);
}

#[test]
fn clone_update_generates_valid_wat() {
    let source = r#"
record Profile {
    id: Int32,
    display_name: String,
    score: Float64,
    active: Boolean
}

fun apply_patch: (base: Profile, patch: Profile) -> Profile = {
    base.clone {
        ...patch,
        active: true,
        display_name: "patched"
    }
}

fun main: () -> Float64 = {
    val base = Profile {
        id: 1,
        display_name: "base",
        score: 10.0,
        active: false
    };
    val patch = Profile {
        id: 2,
        display_name: "patch",
        score: 22.5,
        active: false
    };
    val updated = (base, patch) apply_patch;
    updated.score
}
"#;

    assert_valid_wat("clone_update", source);
}

#[test]
fn float_list_literal_generates_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val readings: List<Float64> = [1.0, 2.0, 3.5];
    0
}
"#;

    assert_valid_wat("float_list_literal", source);
}

#[test]
fn float_list_get_generates_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val readings: List<Float64> = [1.0, 2.0, 3.5];
    val second = (readings, 1) list_get;
    second + 1.0
}
"#;

    assert_valid_wat("float_list_get", source);
}

#[test]
fn mutable_float_list_length_and_get_generate_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    mut val readings: List<Float64> = [1.0, 2.0, 3.5];
    val count = readings |> list_length;
    val second = (readings, 1) list_get;
    val has_samples = count > 0;
    has_samples then {
        second
    } else {
        0.0
    }
}
"#;

    assert_valid_wat("mutable_float_list_length_and_get", source);
}

#[test]
fn arena_allocator_generates_bounds_checked_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val readings = [1, 2, 3];
    val first = (readings, 0) list_get;
    first
}
"#;

    let wat = compile_to_wat(source).expect("arena allocation example should compile");
    assert!(
        wat.contains(";; Arena bounds check"),
        "arena allocator should contain a bounds check:\n{}",
        wat
    );
    assert!(
        wat.contains(";; Arena allocation overflow - trap"),
        "arena allocator should trap on overflow:\n{}",
        wat
    );
    assert_valid_wat("arena_allocator_bounds_checked", source);
}

#[test]
fn logical_boolean_ops_generate_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val both = true && false;
    val either = both || true;
    either then {
        1
    } else {
        0
    }
}
"#;

    assert_valid_wat("logical_boolean_ops", source);
}

#[test]
fn unary_ops_generate_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val negative = -1.5;
    val int_negative = -3;
    val ready = !(int_negative > 0);
    ready then {
        negative + 2.0
    } else {
        0.0
    }
}
"#;

    assert_valid_wat("unary_ops", source);
}

#[test]
fn spec_integer_literal_forms_generate_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val hex = 0xFF;
    val grouped = 1_000_000;
    hex + grouped
}
"#;

    let wat = assert_valid_wat("spec_integer_literal_forms", source);

    assert!(
        wat.contains("i32.const 255"),
        "hex integer literal should lower to decimal i32 constant:\n{wat}"
    );
    assert!(
        wat.contains("i32.const 1000000"),
        "underscored integer literal should lower without separators:\n{wat}"
    );
}

#[test]
fn spec_exponent_float_literal_forms_generate_valid_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val large = 1.5e10;
    val small = 3.14E-2;
    large + small
}
"#;

    let wat = assert_valid_wat("spec_exponent_float_literal_forms", source);

    assert!(
        wat.contains("f64.const 15000000000"),
        "lowercase exponent float literal should lower to f64 constant:\n{wat}"
    );
    assert!(
        wat.contains("f64.const 0.0314"),
        "uppercase signed exponent float literal should lower to f64 constant:\n{wat}"
    );
}

#[test]
fn spec_string_escape_literal_forms_generate_valid_wat() {
    let source = r#"
fun main: () -> String = {
    val message = "a\nb\t\\\"\'";
    message
}
"#;

    let wat = assert_valid_wat("spec_string_escape_literal_forms", source);

    assert!(
        wat.contains(r#"\07\00\00\00a\0ab\09"#),
        "escaped newline and tab should be encoded in string data:\n{wat}"
    );
    assert!(
        wat.contains(r#"\\\"'"#),
        "escaped backslash, quote, and apostrophe should be encoded in string data:\n{wat}"
    );
}

#[test]
fn spec_char_escape_literal_forms_generate_valid_wat() {
    let source = r#"
fun classify: (code: Char) -> Int32 = {
    code match {
        '\n' => {
            10
        }
        '\t' => {
            9
        }
        '\\' => {
            92
        }
        '\'' => {
            39
        }
        _ => {
            0
        }
    }
}

fun main: () -> Int32 = {
    val newline = '\n' |> classify;
    val tab = '\t' |> classify;
    val slash = '\\' |> classify;
    val quote = '\'' |> classify;
    newline + tab + slash + quote
}
"#;

    assert_valid_wat("spec_char_escape_literal_forms", source);
}

#[test]
fn char_literals_and_patterns_generate_valid_wat() {
    let source = r#"
record Badge {
    code: Char
}

fun classify: (code: Char) -> Int32 = {
    code match {
        'A' => {
            10
        }
        _ => {
            0
        }
    }
}

fun main: () -> Int32 = {
    val badge = Badge { code: 'A' };
    val score = 'A' |> classify;
    val record_score = badge match {
        Badge { code: 'A' } => {
            1
        }
        _ => {
            0
        }
    };
    score + record_score
}
"#;

    assert_valid_wat("char_literals_and_patterns", source);
}

#[test]
fn string_and_float_literal_patterns_generate_valid_wat() {
    let source = r#"
record Route {
    status: String,
    load: Float64
}

fun status_points: (status: String) -> Int32 = {
    status match {
        "ok" => {
            0
        }
        "page" => {
            10
        }
        _ => {
            1
        }
    }
}

fun load_points: (load: Float64) -> Int32 = {
    load match {
        0.0 => {
            0
        }
        1.0 => {
            5
        }
        _ => {
            2
        }
    }
}

fun main: () -> Int32 = {
    val route = Route { status: "page", load: 1.0 };
    val by_field = route match {
        Route { status: "page", load: 1.0 } => {
            7
        }
        _ => {
            0
        }
    };
    val status_score = "page" |> status_points;
    val load_score = 1.0 |> load_points;
    by_field + status_score + load_score
}
"#;

    assert_valid_wat("string_and_float_literal_patterns", source);
}

#[test]
fn mutable_while_loop_generates_valid_wat() {
    assert_valid_wat("retry_budget", include_str!("../examples/retry_budget.rl"));
}

#[test]
fn mutable_int64_assignment_generates_valid_wat() {
    let source = r#"
fun main: () -> Int64 = {
    mut val score: Int64 = 0;
    score = 1;
    score
}
"#;

    let wat = assert_valid_wat("mutable_int64_assignment", source);
    assert!(
        wat.contains("i64.const 1"),
        "Int64 assignment should emit the RHS literal with the target ABI:\n{wat}"
    );
}

#[test]
fn float_match_arms_generate_valid_wat() {
    let source = r#"
fun threshold_bonus: (exceeded: Boolean) -> Float64 = {
    exceeded match {
        true => { 1.25 }
        false => { 0.0 }
    }
}

fun main: () -> Float64 = {
    true |> threshold_bonus
}
"#;

    assert_valid_wat("float_match_arms", source);
}

#[test]
fn identifier_match_binding_generates_valid_wat() {
    let source = r#"
fun bump: (value: Int32) -> Int32 = {
    value match {
        0 => { 1 }
        n => { n + 1 }
    }
}

fun main: () -> Int32 = {
    41 |> bump
}
"#;

    assert_valid_wat("identifier_match_binding", source);
}

#[test]
fn float_record_match_destructuring_generates_valid_wat() {
    let source = r#"
record Reading {
    measured: Float64,
    expected: Float64
}

fun drift: (reading: Reading) -> Float64 = {
    reading match {
        Reading { measured, expected } => {
            measured - expected
        }
    }
}

fun main: () -> Float64 = {
    val reading = Reading { measured: 73.8, expected: 72.0 };
    reading |> drift
}
"#;

    assert_valid_wat("float_record_match_destructuring", source);
}

#[test]
fn option_float_payload_match_generates_valid_wat() {
    let source = r#"
fun unwrap_or_zero: (maybe: Option<Float64>) -> Float64 = {
    maybe match {
        Some(value) => {
            value
        }
        None => {
            0.0
        }
    }
}

fun main: () -> Float64 = {
    val maybe: Option<Float64> = Some(1.5);
    maybe |> unwrap_or_zero
}
"#;

    assert_valid_wat("option_float_payload_match", source);
}

#[test]
fn result_float_payload_match_generates_valid_wat() {
    let source = r#"
fun decode_score: (result: Result<Float64, Int32>) -> Float64 = {
    result match {
        Ok(value) => {
            value
        }
        Err(code) => {
            0.0
        }
    }
}

fun main: () -> Float64 = {
    val result: Result<Float64, Int32> = Ok(2.5);
    result |> decode_score
}
"#;

    assert_valid_wat("result_float_payload_match", source);
}

#[test]
fn function_returned_option_float_payload_match_generates_valid_wat() {
    let source = r#"
fun choose_offset: (enabled: Boolean) -> Option<Float64> = {
    enabled then {
        Some(0.25)
    } else {
        None
    }
}

fun main: () -> Float64 = {
    val offset = true |> choose_offset;
    offset match {
        Some(amount) => {
            amount
        }
        None => {
            0.0
        }
    }
}
"#;

    assert_valid_wat("function_returned_option_float_payload_match", source);
}

#[test]
fn function_returned_result_float_payload_match_generates_valid_wat() {
    let source = r#"
fun decode: (ok: Boolean) -> Result<Float64, Int32> = {
    ok then {
        Ok(2.5)
    } else {
        Err(1)
    }
}

fun main: () -> Float64 = {
    val result = true |> decode;
    result match {
        Ok(value) => {
            value
        }
        Err(code) => {
            0.0
        }
    }
}
"#;

    assert_valid_wat("function_returned_result_float_payload_match", source);
}

#[test]
fn lambda_with_inner_arena_scope_generates_valid_wat() {
    let source = r#"
fun main: () -> Int32 = {
    with Arena {
        val score: Int32 -> Int32 = |base| {
            with Arena {
                val readings: List<Int32> = [base, base + 1];
                readings |> list_count
            }
        };
        41 |> score
    }
}
"#;

    let wat = assert_valid_wat("lambda_with_inner_arena_scope", source);
    let lambda_start = wat
        .find("(func $lambda_")
        .expect("lambda function should be emitted");
    let lambda_wat = &wat[lambda_start..];

    assert!(
        lambda_wat.contains("(local $with_prev_arena_0 i32)"),
        "lambda functions should declare Arena restore scratch locals:\n{lambda_wat}"
    );
    assert!(
        lambda_wat.contains("local.set $with_prev_arena_0"),
        "lambda-local `with Arena` should start at depth 0 even when the lambda is created inside another Arena scope:\n{lambda_wat}"
    );
    assert!(
        !lambda_wat.contains("local.set $with_prev_arena_1"),
        "lambda codegen should not inherit the outer function's Arena nesting depth:\n{lambda_wat}"
    );
}

#[test]
fn float_and_int64_array_update_functions_generate_valid_wat() {
    let source = r#"
fun update_float: () -> Float64 = {
    with Arena {
        mut val arr: Array<Float64, 2> = [1.5, 2.5];
        (arr, 0, 3.5) array_set;
        (arr, 0) array_get
    }
}

fun update_int64: () -> Int64 = {
    with Arena {
        mut val arr: Array<Int64, 2> = [10000000000, 20000000000];
        (arr, 1, 30000000000) array_set;
        (arr, 1) array_get
    }
}

fun main: () -> Int64 = {
    () update_int64
}
"#;

    let wat = assert_valid_wat("float_and_int64_array_update_functions", source);
    assert!(wat.contains("call $array_set_f64"));
    assert!(wat.contains("call $array_get_f64"));
    assert!(wat.contains("call $array_set_i64"));
    assert!(wat.contains("call $array_get_i64"));
    assert!(
        wat.contains("call $array_bounds_check"),
        "Array get/set helpers should validate indexes at runtime:\n{wat}"
    );
    assert!(
        wat.contains(";; Array bounds check: index >= length traps"),
        "Array helper should contain an explicit bounds check:\n{wat}"
    );
    assert!(
        wat.contains("i32.load ;; length") && wat.contains(";; Array index out of bounds - trap"),
        "Array bounds check should load the header length and trap on failure:\n{wat}"
    );
    assert!(
        wat.contains("array + 8 + (index * 4)"),
        "Int32 Array payload addressing should account for the length/header words:\n{wat}"
    );
    assert!(
        wat.contains("array + 8 + (index * 8)"),
        "Float64/Int64 Array payload addressing should account for the length/header words:\n{wat}"
    );
}

#[test]
fn unannotated_later_use_float_and_int64_arrays_generate_typed_helpers() {
    let source = r#"
fun read_float: () -> Float64 = {
    with Arena {
        val readings = [1.5, 2.5];
        (readings, 1) array_get
    }
}

fun update_empty_float: () -> Float64 = {
    with Arena {
        val readings = [];
        (readings, 0, 3.5) array_set;
        (readings, 0) array_get
    }
}

fun read_int64: () -> Int64 = {
    with Arena {
        val counters = [10000000000, 20000000000];
        (counters, 1) array_get
    }
}

fun main: () -> Int64 = {
    () read_int64
}
"#;

    let wat = assert_valid_wat("unannotated_later_use_float_and_int64_arrays", source);
    assert!(
        wat.contains("call $array_get_f64"),
        "Float64 later-use array inference should select the f64 ABI helper:\n{wat}"
    );
    assert!(
        wat.contains("call $array_set_f64"),
        "Float64 array_set should preserve the inferred Array element ABI:\n{wat}"
    );
    assert!(
        wat.contains("call $array_get_i64"),
        "Int64 later-use array inference should select the i64 ABI helper:\n{wat}"
    );
    assert!(
        wat.contains(";; array size"),
        "Inferred Array locals should be emitted with the Array literal layout:\n{wat}"
    );
    assert!(
        !wat.contains(";; list size"),
        "Inferred Array locals should not fall back to List literal allocation:\n{wat}"
    );
}

#[test]
fn int64_list_update_functions_generate_valid_wat() {
    let source = r#"
fun update_int64_list: () -> Int64 = {
    with Arena {
        val base: List<Int64> = [10000000000, 20000000000];
        val appended = (base, 30000000000) list_append;
        val prepended = (40000000000, appended) list_prepend;
        val reversed = prepended |> list_reverse;
        (reversed, 0) list_get
    }
}

fun read_int64_head: () -> Int64 = {
    with Arena {
        val values: List<Int64> = [50000000000, 60000000000];
        val head = values |> list_head;
        head match {
            Some(value) => {
                value
            }
            None => {
                0
            }
        }
    }
}

fun count_int64_tail: () -> Int32 = {
    with Arena {
        val values: List<Int64> = [70000000000, 80000000000];
        val tail = values |> list_tail;
        tail match {
            Some(rest) => {
                rest |> list_count
            }
            None => {
                0
            }
        }
    }
}

fun main: () -> Int64 = {
    () update_int64_list
}
"#;

    let wat = assert_valid_wat("int64_list_update_functions", source);
    assert!(wat.contains("call $list_append_i64"));
    assert!(wat.contains("call $list_prepend_i64"));
    assert!(wat.contains("call $list_reverse_i64"));
    assert!(wat.contains("call $list_get_i64"));
    assert!(wat.contains("call $list_head_i64"));
    assert!(wat.contains("call $list_tail_i64"));
    assert!(wat.contains("call $tail_i64"));
}

#[test]
fn tail_alias_uses_int64_helper() {
    let source = r#"
fun main: () -> Int64 = {
    with Arena {
        val values: List<Int64> = [10000000000, 20000000000, 30000000000];
        val rest = values |> tail;
        (rest, 0) list_get
    }
}
"#;

    let wat = assert_valid_wat("tail_alias_int64", source);
    assert!(
        wat.contains("call $tail_i64"),
        "Int64 tail should use the i64 ABI helper:\n{wat}"
    );
    assert!(
        wat.contains("call $list_get_i64"),
        "Int64 tail result should keep the i64 List ABI:\n{wat}"
    );
}

#[test]
fn small_int64_collection_literals_and_unit_binding_generate_valid_wat() {
    let source = r#"
fun update_small_int64_array: () -> Int64 = {
    with Arena {
        mut val arr: Array<Int64, 2> = [1, 2];
        val done = (arr, 1, 3) array_set;
        (arr, 1) array_get
    }
}

fun update_small_int64_list: () -> Int64 = {
    with Arena {
        val base: List<Int64> = [1, 2];
        val appended = (base, 3) list_append;
        (appended, 2) list_get
    }
}

fun match_small_int64_default: () -> Int64 = {
    with Arena {
        val values: List<Int64> = [];
        val head = values |> list_head;
        head match {
            Some(value) => {
                value
            }
            None => {
                0
            }
        }
    }
}

fun main: () -> Int64 = {
    () update_small_int64_array
}
"#;

    let wat = assert_valid_wat("small_int64_collection_literals_and_unit_binding", source);
    assert!(wat.contains("call $array_set_i64"));
    assert!(wat.contains("call $array_get_i64"));
    assert!(wat.contains("call $list_append_i64"));
    assert!(wat.contains("call $list_get_i64"));
    assert!(wat.contains("call $list_head_i64"));
}
