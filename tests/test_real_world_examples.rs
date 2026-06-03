use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use std::fs;
use std::process::Command;
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

const PROMOTED_DOGFOOD_RELEASE_EXAMPLES: &[&str] = &[
    "examples/dogfood_branch_callable_prefix_inference.rl",
    "examples/dogfood_ci_test_planner_inference.rl",
    "examples/dogfood_inference_task_queue.rl",
    "examples/dogfood_metrics_rollup_inference.rl",
    "examples/dogfood_release_readiness_inference.rl",
    "examples/dogfood_release_patch_inference.rl",
    "examples/dogfood_result_local_inference.rl",
    "examples/dogfood_mutable_checkpoint_inference.rl",
    "examples/dogfood_spec_literals_inference.rl",
    "examples/dogfood_array_range_window_inference.rl",
];

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

fn type_check_source(source: &str) -> Result<(), String> {
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))
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

#[test]
fn promoted_dogfood_release_examples_compile_to_valid_wat() {
    for example_path in PROMOTED_DOGFOOD_RELEASE_EXAMPLES {
        let source = fs::read_to_string(example_path).expect("example should be readable");
        let wat = compile_to_wat(&source)
            .unwrap_or_else(|err| panic!("{example_path} should compile to WAT: {err}"));

        assert!(
            wat.trim_start().starts_with("(module"),
            "{example_path} should generate a WAT module"
        );
        wat::parse_str(&wat)
            .unwrap_or_else(|err| panic!("{example_path} generated invalid WAT: {err}\n\n{wat}"));
    }
}

#[test]
fn order_pricing_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/order_pricing.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("order pricing example should compile");

    assert!(wat.contains("(func $clamp_discount"));
    assert!(wat.contains("(func $expedited_fee"));
    assert!(wat.contains("(func $total_due"));
    assert!(wat.contains("call $total_due"));
}

#[test]
fn lambda_uses_expected_function_type() {
    let source = r#"
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    (|x| x * 2, 21) apply_int
}
"#;

    type_check_source(source).expect("lambda should use expected function type");
}

#[test]
fn lambda_expected_inference_example_type_checks() {
    let source = fs::read_to_string("examples/lambda_expected_inference.rl")
        .expect("example should be readable");

    type_check_source(&source).expect("lambda expected-type example should type check");
}

#[test]
fn lambda_inference_example_type_checks() {
    let source =
        fs::read_to_string("examples/lambda_inference.rl").expect("example should be readable");

    type_check_source(&source).expect("lambda inference example should type check");
}

#[test]
fn list_example_type_checks() {
    let source =
        fs::read_to_string("examples/list_example.rl").expect("example should be readable");

    type_check_source(&source).expect("list example should type check");
}

#[test]
fn comments_example_type_checks() {
    let source =
        fs::read_to_string("examples/test_comments.rl").expect("example should be readable");

    type_check_source(&source).expect("comment example should type check");
}

#[test]
fn return_annotation_contract_example_type_checks() {
    let source = fs::read_to_string("examples/return_annotation_contract.rl")
        .expect("example should be readable");

    type_check_source(&source).expect("return annotation contract example should type check");
}

#[test]
fn checkout_review_example_type_checks() {
    let source =
        fs::read_to_string("examples/checkout_review.rl").expect("example should be readable");

    type_check_source(&source).expect("checkout review example should type check");
}

#[test]
fn deploy_gate_example_type_checks() {
    let source = fs::read_to_string("examples/deploy_gate.rl").expect("example should be readable");

    type_check_source(&source).expect("deploy gate example should type check");
}

#[test]
fn incident_triage_example_type_checks() {
    let source =
        fs::read_to_string("examples/incident_triage.rl").expect("example should be readable");

    type_check_source(&source).expect("incident triage example should type check");
}

#[test]
fn fulfillment_batch_example_type_checks() {
    let source =
        fs::read_to_string("examples/fulfillment_batch.rl").expect("example should be readable");

    type_check_source(&source).expect("fulfillment batch example should type check");
}

#[test]
fn inventory_reorder_example_type_checks() {
    let source =
        fs::read_to_string("examples/inventory_reorder.rl").expect("example should be readable");

    type_check_source(&source).expect("inventory reorder example should type check");
}

#[test]
fn sensor_calibration_example_type_checks() {
    let source =
        fs::read_to_string("examples/sensor_calibration.rl").expect("example should be readable");

    type_check_source(&source).expect("sensor calibration example should type check");
}

#[test]
fn status_routing_example_type_checks() {
    let source =
        fs::read_to_string("examples/status_routing.rl").expect("example should be readable");

    type_check_source(&source).expect("status routing example should type check");
}

#[test]
fn result_validation_example_type_checks() {
    let source =
        fs::read_to_string("examples/result_validation.rl").expect("example should be readable");

    type_check_source(&source).expect("result validation example should type check");
}

#[test]
fn retry_budget_example_type_checks() {
    let source =
        fs::read_to_string("examples/retry_budget.rl").expect("example should be readable");

    type_check_source(&source).expect("retry budget example should type check");
}

#[test]
fn sprint_planner_example_type_checks() {
    let source =
        fs::read_to_string("examples/sprint_planner.rl").expect("example should be readable");

    type_check_source(&source).expect("sprint planner example should type check");
}

#[test]
fn release_readiness_example_type_checks() {
    let source =
        fs::read_to_string("examples/release_readiness.rl").expect("example should be readable");

    type_check_source(&source).expect("release readiness example should type check");
}

#[test]
fn release_queue_snapshot_example_compiles_to_wat() {
    let source = fs::read_to_string("examples/release_queue_snapshot.rl")
        .expect("example should be readable");

    let wat = compile_to_wat(&source).expect("release queue snapshot example should compile");

    assert!(wat.contains("(func $candidate_score"));
    assert!(wat.contains("(func $score_candidate_option"));
    assert!(wat.contains("(func $count_tail_option"));
    assert!(wat.contains("call $list_reverse"));
}

#[test]
fn release_queue_snapshot_example_executes() {
    let source = fs::read_to_string("examples/release_queue_snapshot.rl")
        .expect("example should be readable")
        .replace(
            "fun main: () -> Int32",
            "export fun release_queue_snapshot_score: () -> Int32",
        );
    let wat = compile_to_wat(&source)
        .expect("release queue snapshot runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("release queue snapshot runtime", &wat);
    let release_queue_snapshot_score = instance
        .get_typed_func::<(), i32>(&store, "release_queue_snapshot_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        release_queue_snapshot_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        27
    );
}

#[test]
fn support_queue_example_type_checks() {
    let source =
        fs::read_to_string("examples/support_queue.rl").expect("example should be readable");

    type_check_source(&source).expect("support queue example should type check");
}

#[test]
fn service_monitor_example_type_checks() {
    let source =
        fs::read_to_string("examples/service_monitor.rl").expect("example should be readable");

    type_check_source(&source).expect("service monitor example should type check");
}

#[test]
fn rollout_handoff_example_type_checks() {
    let source =
        fs::read_to_string("examples/rollout_handoff.rl").expect("example should be readable");

    type_check_source(&source).expect("rollout handoff example should type check");
}

#[test]
fn context_policy_gate_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/context_policy_gate.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("context policy gate example should compile");

    assert!(wat.contains("(func $adjusted_score"));
    assert!(wat.contains("local.set $minimum_score"));
    assert!(wat.contains("local.set $risk_penalty"));
    assert!(wat.contains("call $adjusted_score"));
}

#[test]
fn feature_rollout_policy_example_compiles_to_wat() {
    let source = fs::read_to_string("examples/feature_rollout_policy.rl")
        .expect("example should be readable");

    let wat = compile_to_wat(&source).expect("feature rollout policy example should compile");

    assert!(wat.contains("(func $classify_feature"));
    assert!(wat.contains("(func $decision_priority"));
    assert!(wat.contains("call $classify_feature"));
}

#[test]
fn subscription_billing_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/subscription_billing.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("subscription billing example should compile");

    assert!(wat.contains("(func $default_credits"));
    assert!(wat.contains("(func $coupon_discount"));
    assert!(wat.contains("(func $compute_invoice"));
    assert!(wat.contains("call $coupon_discount"));
}

#[test]
fn bug_triage_board_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/bug_triage_board.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("bug triage board example should compile");

    assert!(wat.contains("(func $build_board"));
    assert!(wat.contains("(func $add_ticket"));
    assert!(wat.contains("(func $signal_points"));
    assert!(wat.contains("call $add_ticket"));
}

#[test]
fn review_policy_factory_example_compiles_to_wat() {
    let source = fs::read_to_string("examples/review_policy_factory.rl")
        .expect("example should be readable");

    let wat = compile_to_wat(&source).expect("review policy factory example should compile");

    assert!(wat.contains("(func $make_policy"));
    assert!(wat.contains("(func $decide_review"));
    assert!(wat.contains("(func $lambda_"));
    assert!(wat.contains("call_indirect"));
}

#[test]
fn change_review_gate_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/change_review_gate.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("change review gate example should compile");

    assert!(wat.contains("(func $assess_change"));
    assert!(wat.contains(";; map(option, mapper)"));
    assert!(wat.contains(";; filter(option, predicate)"));
    assert!(wat.contains("call_indirect"));
}

#[test]
fn calibration_pipeline_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/calibration_pipeline.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("calibration pipeline example should compile");

    assert!(wat.contains("(func $quality_score"));
    assert!(wat.contains("(func $normalize_sample"));
    assert!(wat.contains("closure_call_1_i32_to_f64"));
    assert!(wat.contains("closure_call_1_f64_to_i32"));
    assert!(wat.contains("closure_call_2_f64_f64_to_f64"));
    assert!(wat.contains(";; map(list, mapper)"));
    assert!(wat.contains(";; filter(list, predicate)"));
    assert!(wat.contains(";; fold(list, initial, reducer)"));
}

#[test]
fn experiment_scorecard_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/experiment_scorecard.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("experiment scorecard example should compile");

    assert!(wat.contains("(func $evaluate_experiment"));
    assert!(wat.contains("(func $make_metric_scorer"));
    assert!(wat.contains("closure_call_1_i32_to_f64"));
    assert!(wat.contains("closure_call_1_f64_to_i32"));
    assert!(wat.contains("closure_call_2_f64_f64_to_f64"));
    assert!(wat.contains(";; map(list, mapper)"));
    assert!(wat.contains(";; filter(list, predicate)"));
    assert!(wat.contains(";; fold(list, initial, reducer)"));
    assert!(wat.contains(";; map(option, mapper)"));
    assert!(wat.contains(";; filter(option, predicate)"));
}

#[test]
fn release_review_digest_example_compiles_to_wat() {
    let source = fs::read_to_string("examples/release_review_digest.rl")
        .expect("example should be readable");

    let wat = compile_to_wat(&source).expect("release review digest example should compile");

    assert!(wat.contains("(func $summarize_slice"));
    assert!(wat.contains("local.set $record_tmp_0"));
    assert!(wat.contains("local.set $record_tmp_1"));
}

#[test]
fn release_review_digest_example_executes() {
    let digest_source = fs::read_to_string("examples/release_review_digest.rl")
        .expect("example should be readable")
        .replace(
            "fun main: () -> Digest",
            "fun build_release_review_digest: () -> Digest",
        );
    let source = format!(
        "{}\n{}",
        digest_source,
        r#"
export fun release_review_digest_score: () -> Int32 = {
    val digest = () build_release_review_digest;
    val Digest {
        priority,
        confidence,
        first_blocker,
        second_blocker
    } = digest;

    priority + confidence + first_blocker + second_blocker
}
"#
    );
    let wat =
        compile_to_wat(&source).expect("release review digest runtime wrapper should compile");

    let (mut store, instance) = instantiate_wat("release review digest runtime", &wat);
    let release_review_digest_score = instance
        .get_typed_func::<(), i32>(&store, "release_review_digest_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        release_review_digest_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        321
    );
}

#[test]
fn release_decision_engine_example_compiles_to_wat() {
    let source = fs::read_to_string("examples/release_decision_engine.rl")
        .expect("example should be readable");

    let wat = compile_to_wat(&source).expect("release decision engine example should compile");

    assert!(wat.contains("(global $default_owner_id"));
    assert!(wat.contains("(global $default_risk_limit"));
    assert!(wat.contains("(func $decide_release"));
    assert!(wat.contains(";; map(list, mapper)"));
    assert!(wat.contains(";; filter(list, predicate)"));
    assert!(wat.contains(";; fold(list, initial, reducer)"));
}

#[test]
fn release_decision_engine_example_executes() {
    let decision_source = fs::read_to_string("examples/release_decision_engine.rl")
        .expect("example should be readable")
        .replace(
            "fun main: () -> ReleaseDecision",
            "fun build_release_decision: () -> ReleaseDecision",
        );
    let source = format!(
        "{}\n{}",
        decision_source,
        r#"
export fun release_decision_engine_score: () -> Int32 = {
    val decision = () build_release_decision;
    val ReleaseDecision {
        risk,
        blocked,
        primary_owner,
        first_blocker,
        audit_scores
    } = decision;
    val blocked_score = blocked then {
        100
    } else {
        0
    };
    val audit_score_count = audit_scores |> list_count;

    primary_owner + first_blocker + blocked_score + audit_score_count
}
"#
    );
    let wat =
        compile_to_wat(&source).expect("release decision engine runtime wrapper should compile");

    let (mut store, instance) = instantiate_wat("release decision engine runtime", &wat);
    let release_decision_engine_score = instance
        .get_typed_func::<(), i32>(&store, "release_decision_engine_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        release_decision_engine_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        43
    );
}

#[test]
fn typed_impl_dispatch_example_compiles_to_wat() {
    let source =
        fs::read_to_string("examples/typed_impl_dispatch.rl").expect("example should be readable");

    let wat = compile_to_wat(&source).expect("typed impl dispatch example should compile");

    assert!(wat.contains("(func $HealthSignal_risk_score"));
    assert!(wat.contains("(func $RolloutSignal_risk_score"));
    assert!(wat.contains("call $HealthSignal_risk_score"));
    assert!(wat.contains("call $RolloutSignal_risk_score"));
    assert!(wat.contains("(func $decide_dispatch"));
}

#[test]
fn typed_impl_dispatch_example_executes() {
    let dispatch_source = fs::read_to_string("examples/typed_impl_dispatch.rl")
        .expect("example should be readable")
        .replace(
            "fun main: () -> DispatchDecision",
            "fun build_typed_impl_dispatch: () -> DispatchDecision",
        );

    let source = format!(
        "{dispatch_source}\n{}",
        r#"
export fun typed_impl_dispatch_score: () -> Float64 = {
    val decision = () build_typed_impl_dispatch;
    val DispatchDecision { health_risk, rollout_risk, approved } = decision;

    approved then {
        health_risk + rollout_risk
    } else {
        0.0
    }
}
"#
    );

    let wat = compile_to_wat(&source).expect("typed impl dispatch runtime wrapper should compile");

    let (mut store, instance) = instantiate_wat("typed impl dispatch runtime", &wat);
    let score = instance
        .get_typed_func::<(), f64>(&store, "typed_impl_dispatch_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        69.5
    );
}

#[test]
fn modular_release_gate_example_compiles_with_imports() {
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_modular_release_gate_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("examples/modular_release_gate.rl")
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "modular release gate example should compile through the CLI, stderr: {}",
        stderr
    );

    let wat = fs::read_to_string(&output_path).expect("compiled WAT should be readable");
    assert!(wat.contains("(func $evaluate_release"));
    assert!(wat.contains("(func $__rl_mod_modules_release_scores_score_signal"));
    assert!(wat.contains("call $evaluate_release"));
    assert!(wat.contains("call $__rl_mod_modules_release_scores_score_signal"));
    assert!(wat.contains(";; map(list, mapper)"));
    assert!(wat.contains(";; fold(list, initial, reducer)"));

    let (mut store, instance) = instantiate_wat("modular release gate runtime", &wat);
    let modular_release_gate_score = instance
        .get_typed_func::<(), i32>(&store, "modular_release_gate_score")
        .expect("modular release gate wrapper should be host-callable");
    assert_eq!(
        modular_release_gate_score
            .call(&mut store, ())
            .expect("modular release gate wrapper should execute"),
        63
    );

    let _ = fs::remove_file(output_path);
}

#[test]
fn modular_release_import_surface_example_compiles_with_whole_module_and_wildcard_imports() {
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_modular_release_import_surface_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("examples/modular_release_import_surface.rl")
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "whole-module and wildcard import release example should compile through the CLI, stderr: {}",
        stderr
    );

    let wat = fs::read_to_string(&output_path).expect("compiled WAT should be readable");
    assert!(wat.contains("(func $evaluate_release"));
    assert!(wat.contains("(func $__rl_mod_modules_release_scores_score_signal"));
    assert!(wat.contains("call $evaluate_release"));
    assert!(wat.contains("call $__rl_mod_modules_release_scores_score_signal"));

    let (mut store, instance) = instantiate_wat("modular release import surface runtime", &wat);
    let modular_release_import_surface_score = instance
        .get_typed_func::<(), i32>(&store, "modular_release_import_surface_score")
        .expect("modular release import surface wrapper should be host-callable");
    assert_eq!(
        modular_release_import_surface_score
            .call(&mut store, ())
            .expect("modular release import surface wrapper should execute"),
        140
    );

    let _ = fs::remove_file(output_path);
}

#[test]
fn modular_policy_context_gate_example_compiles_with_imported_context_and_impl_dispatch() {
    let output_path = std::env::temp_dir().join(format!(
        "restrict_lang_modular_policy_context_gate_{}.wat",
        std::process::id()
    ));
    let _ = fs::remove_file(&output_path);

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("examples/modular_policy_context_gate.rl")
        .arg(&output_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "modular policy-context gate example should compile through the CLI, stderr: {}",
        stderr
    );

    let wat = fs::read_to_string(&output_path).expect("compiled WAT should be readable");
    assert!(wat.contains("(func $modular_policy_context_score"));
    assert!(wat.contains("(func $decide_review"));
    assert!(wat.contains("call $decide_review"));
    assert!(wat.contains("ReviewSignal_policy_penalty"));
    assert!(wat.contains("RolloutSignal_policy_penalty"));

    let (mut store, instance) = instantiate_wat("modular policy-context gate runtime", &wat);
    let modular_policy_context_score = instance
        .get_typed_func::<(), i32>(&store, "modular_policy_context_score")
        .expect("modular policy-context wrapper should be host-callable");
    assert_eq!(
        modular_policy_context_score
            .call(&mut store, ())
            .expect("modular policy-context wrapper should execute"),
        50
    );

    let _ = fs::remove_file(output_path);
}

#[test]
fn record_literals_push_expected_types_into_fields() {
    let source = r#"
record Envelope {
    attempts: List<Int32>,
    notes: Option<List<Int32>>,
    fallback: Option<Int32>
}

fun main: () -> Envelope = {
    Envelope {
        attempts: [],
        notes: Some([]),
        fallback: None
    }
}
"#;

    type_check_source(source).expect("record fields should provide expected types");
}

#[test]
fn record_literal_missing_field_is_rejected() {
    let source = r#"
record Envelope {
    attempts: List<Int32>,
    fallback: Option<Int32>
}

fun main: () -> Envelope = {
    Envelope {
        attempts: []
    }
}
"#;

    let err = type_check_source(source).expect_err("record literal should require all fields");
    assert!(
        err.contains("Missing field"),
        "error should explain the missing field, got: {}",
        err
    );
}

#[test]
fn record_spread_requires_same_record_type() {
    let source = r#"
record Profile {
    id: Int32,
    score: Int32
}

record Patch {
    id: Int32,
    score: Int32
}

fun main: () -> Profile = {
    val patch = Patch { id: 1, score: 20 };
    Profile {
        ...patch
    }
}
"#;

    let err = type_check_source(source).expect_err("record spread should require same type");
    assert!(
        err.contains("expected record Profile"),
        "error should explain same-record spread requirement, got: {}",
        err
    );
}

#[test]
fn record_spread_preserving_noncopy_field_is_rejected() {
    let source = r#"
record Profile {
    name: String,
    score: Int32
}

fun main: () -> Profile = {
    val base = Profile { name: "Ada", score: 10 };
    Profile {
        ...base,
        score: 20
    }
}
"#;

    let err =
        type_check_source(source).expect_err("spread should not implicitly copy String fields");
    assert!(
        err.contains("record spread would implicitly copy non-copy field Profile.name"),
        "error should identify the affine field copied by spread, got: {err}"
    );
}

#[test]
fn record_spread_replacing_noncopy_field_is_allowed() {
    let source = r#"
record Profile {
    name: String,
    score: Int32
}

fun main: () -> Profile = {
    val base = Profile { name: "Ada", score: 10 };
    Profile {
        ...base,
        name: "Grace",
        score: 20
    }
}
"#;

    type_check_source(source)
        .expect("spread may preserve copy fields when affine fields are replaced");
}

#[test]
fn clone_spread_requires_same_record_type() {
    let source = r#"
record Profile {
    id: Int32,
    score: Int32
}

record Patch {
    id: Int32,
    score: Int32
}

fun main: () -> Profile = {
    val base = Profile { id: 1, score: 10 };
    val patch = Patch { id: 1, score: 20 };
    base.clone {
        ...patch
    }
}
"#;

    let err = type_check_source(source).expect_err("clone spread should require same type");
    assert!(
        err.contains("expected record Profile"),
        "error should explain same-record clone spread requirement, got: {}",
        err
    );
}

#[test]
fn clone_preserving_noncopy_field_is_rejected() {
    let source = r#"
record Profile {
    name: String,
    score: Int32
}

fun main: () -> Profile = {
    val base = Profile { name: "Ada", score: 10 };
    base.clone {
        score: 20
    }
}
"#;

    let err =
        type_check_source(source).expect_err("clone should not implicitly copy String fields");
    assert!(
        err.contains("record clone would implicitly copy non-copy field Profile.name"),
        "error should identify the affine field copied by clone, got: {err}"
    );
}

#[test]
fn clone_spread_preserving_noncopy_field_is_rejected() {
    let source = r#"
record Profile {
    name: String,
    score: Int32
}

fun main: () -> Profile = {
    val base = Profile { name: "Ada", score: 10 };
    val patch = Profile { name: "Grace", score: 20 };
    base.clone {
        ...patch,
        score: 30
    }
}
"#;

    let err =
        type_check_source(source).expect_err("clone spread should not preserve String fields");
    assert!(
        err.contains("record clone would implicitly copy non-copy field Profile.name"),
        "error should identify the affine field copied by clone spread, got: {err}"
    );
}

#[test]
fn clone_replacing_noncopy_field_is_allowed() {
    let source = r#"
record Profile {
    name: String,
    score: Int32
}

fun main: () -> Profile = {
    val base = Profile { name: "Ada", score: 10 };
    base.clone {
        name: "Grace",
        score: 20
    }
}
"#;

    type_check_source(source)
        .expect("clone may preserve copy fields when affine fields are replaced");
}

#[test]
fn record_rest_preserving_noncopy_field_is_rejected() {
    let source = r#"
record Profile {
    name: String,
    score: Int32
}

fun main: () -> Int32 = {
    val base = Profile { name: "Ada", score: 10 };
    val Profile { score, ...rest } = base;
    score
}
"#;

    let err = type_check_source(source).expect_err("rest should not implicitly copy String fields");
    assert!(
        err.contains("record rest would implicitly copy non-copy field Profile.name"),
        "error should identify the affine field captured by rest, got: {err}"
    );
}

#[test]
fn record_rest_with_only_copy_fields_is_allowed() {
    let source = r#"
record Profile {
    name: String,
    score: Int32
}

fun main: () -> Int32 = {
    val base = Profile { name: "Ada", score: 10 };
    val Profile { name, ...rest } = base;
    rest.score
}
"#;

    type_check_source(source)
        .expect("rest may retain copyable fields after affine fields are extracted");
}

#[test]
fn clone_updates_push_expected_types_into_fields() {
    let source = r#"
record Envelope {
    attempts: List<Int32>,
    fallback: Option<Int32>
}

fun main: () -> Envelope = {
    val base = Envelope {
        attempts: [1],
        fallback: Some(1)
    };
    base.clone {
        attempts: [],
        fallback: None
    }
}
"#;

    type_check_source(source).expect("clone update fields should provide expected types");
}

#[test]
fn contextless_body_constrained_lambda_is_inferred() {
    let source = r#"
fun main: () -> Int32 = {
    val add_one = |x| x + 1;
    41 |> add_one
}
"#;

    type_check_source(source).expect("body-constrained lambda should infer Int32 -> Int32");
}

#[test]
fn cli_check_rejects_unparsed_tail() {
    let path = std::env::temp_dir().join("restrict_lang_unparsed_tail.rl");
    fs::write(
        &path,
        r#"
fn main() {
    42
}
"#,
    )
    .expect("temp source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "CLI should reject unparsed trailing input"
    );
    assert!(
        stderr.contains("Unparsed input remaining"),
        "stderr should explain the unparsed tail, got: {}",
        stderr
    );
}

#[test]
fn cli_check_resolves_source_imports() {
    let dir =
        std::env::temp_dir().join(format!("restrict_lang_cli_imports_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp module dir should be created");

    fs::write(
        dir.join("release.rl"),
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    )
    .expect("module source should be writable");
    let main_path = dir.join("main.rl");
    fs::write(
        &main_path,
        r#"
import release.{public_score}

fun main: () -> Int32 = {
    41 |> public_score
}
"#,
    )
    .expect("main source should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_lang"))
        .arg("--check")
        .arg(&main_path)
        .output()
        .expect("restrict_lang binary should run");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "CLI should resolve source imports during --check, stderr: {}",
        stderr
    );

    let _ = fs::remove_dir_all(dir);
}
