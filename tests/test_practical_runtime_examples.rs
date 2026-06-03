use restrict_lang::{parse_program, Program, TypeChecker, WasmCodeGen};
use std::fs;
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn parse_source(source: &str) -> Result<Program, String> {
    let (remaining, program) = parse_program(source).map_err(|e| format!("Parse error: {e:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    Ok(program)
}

fn compile_to_wat(source: &str) -> Result<String, String> {
    let program = parse_source(source)?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {e}"))?;

    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&program)
        .map_err(|e| format!("Codegen error: {e}"))
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

fn compile_exported_example(
    example_path: &str,
    main_signature: &str,
    export_signature: &str,
) -> String {
    let source = fs::read_to_string(example_path)
        .unwrap_or_else(|err| panic!("{example_path} should be readable: {err}"))
        .replace(main_signature, export_signature);

    compile_to_wat(&source)
        .unwrap_or_else(|err| panic!("{example_path} runtime wrapper should compile: {err}"))
}

fn compile_example_with_rewritten_main_and_export_wrapper(
    example_path: &str,
    main_signature: &str,
    internal_signature: &str,
    export_wrapper: &str,
) -> String {
    let mut source = fs::read_to_string(example_path)
        .unwrap_or_else(|err| panic!("{example_path} should be readable: {err}"))
        .replace(main_signature, internal_signature);
    source.push('\n');
    source.push_str(export_wrapper);

    compile_to_wat(&source)
        .unwrap_or_else(|err| panic!("{example_path} runtime wrapper should compile: {err}"))
}

fn assert_exported_i32_example(example_path: &str, label: &str, export_name: &str, expected: i32) {
    let export_signature = format!("export fun {export_name}: () -> Int32");
    let wat = compile_exported_example(
        example_path,
        "fun main: () -> Int32",
        export_signature.as_str(),
    );

    let (mut store, instance) = instantiate_wat(label, &wat);
    let func = instance
        .get_typed_func::<(), i32>(&store, export_name)
        .unwrap_or_else(|err| panic!("{export_name} should be host-callable: {err}"));

    assert_eq!(
        func.call(&mut store, ())
            .unwrap_or_else(|err| panic!("{export_name} should execute: {err}")),
        expected
    );
}

fn assert_exported_f64_example(example_path: &str, label: &str, export_name: &str, expected: f64) {
    let export_signature = format!("export fun {export_name}: () -> Float64");
    let wat = compile_exported_example(
        example_path,
        "fun main: () -> Float64",
        export_signature.as_str(),
    );

    let (mut store, instance) = instantiate_wat(label, &wat);
    let func = instance
        .get_typed_func::<(), f64>(&store, export_name)
        .unwrap_or_else(|err| panic!("{export_name} should be host-callable: {err}"));

    let actual = func
        .call(&mut store, ())
        .unwrap_or_else(|err| panic!("{export_name} should execute: {err}"));
    assert!(
        (actual - expected).abs() < 0.000_001,
        "{export_name} should return {expected}, got {actual}"
    );
}

#[test]
fn order_pricing_example_executes() {
    let wat = compile_exported_example(
        "examples/order_pricing.rl",
        "fun main: () -> Int32",
        "export fun order_pricing_total: () -> Int32",
    );

    let (mut store, instance) = instantiate_wat("order pricing runtime", &wat);
    let order_pricing_total = instance
        .get_typed_func::<(), i32>(&store, "order_pricing_total")
        .expect("order pricing wrapper should be host-callable");

    assert_eq!(
        order_pricing_total
            .call(&mut store, ())
            .expect("order pricing wrapper should execute"),
        80
    );
}

#[test]
fn context_policy_gate_example_executes() {
    let wat = compile_exported_example(
        "examples/context_policy_gate.rl",
        "fun main: () -> Float64",
        "export fun context_policy_score: () -> Float64",
    );

    let (mut store, instance) = instantiate_wat("context policy runtime", &wat);
    let context_policy_score = instance
        .get_typed_func::<(), f64>(&store, "context_policy_score")
        .expect("context policy wrapper should be host-callable");

    let score = context_policy_score
        .call(&mut store, ())
        .expect("context policy wrapper should execute");
    assert!(
        (score - 0.8).abs() < 0.000_001,
        "context policy score should clamp to 0.8, got {score}"
    );
}

#[test]
fn feature_rollout_policy_example_executes() {
    let wat = compile_exported_example(
        "examples/feature_rollout_policy.rl",
        "fun main: () -> Int32",
        "export fun feature_rollout_score: () -> Int32",
    );

    let (mut store, instance) = instantiate_wat("feature rollout runtime", &wat);
    let feature_rollout_score = instance
        .get_typed_func::<(), i32>(&store, "feature_rollout_score")
        .expect("feature rollout wrapper should be host-callable");

    assert_eq!(
        feature_rollout_score
            .call(&mut store, ())
            .expect("feature rollout wrapper should execute"),
        23
    );
}

#[test]
fn subscription_billing_example_executes() {
    let wat = compile_exported_example(
        "examples/subscription_billing.rl",
        "fun main: () -> Int32",
        "export fun subscription_billing_total: () -> Int32",
    );

    let (mut store, instance) = instantiate_wat("subscription billing runtime", &wat);
    let subscription_billing_total = instance
        .get_typed_func::<(), i32>(&store, "subscription_billing_total")
        .expect("subscription billing wrapper should be host-callable");

    assert_eq!(
        subscription_billing_total
            .call(&mut store, ())
            .expect("subscription billing wrapper should execute"),
        291
    );
}

#[test]
fn checkout_review_example_executes() {
    assert_exported_i32_example(
        "examples/checkout_review.rl",
        "checkout review runtime",
        "checkout_review_score",
        142,
    );
}

#[test]
fn inventory_reorder_example_executes() {
    assert_exported_i32_example(
        "examples/inventory_reorder.rl",
        "inventory reorder runtime",
        "inventory_reorder_delay",
        4,
    );
}

#[test]
fn status_routing_example_executes() {
    assert_exported_i32_example(
        "examples/status_routing.rl",
        "status routing runtime",
        "status_routing_page",
        1,
    );
}

#[test]
fn result_validation_example_executes() {
    assert_exported_i32_example(
        "examples/result_validation.rl",
        "result validation runtime",
        "fulfillment_result_score",
        123,
    );
}

#[test]
fn retry_budget_example_executes() {
    assert_exported_i32_example(
        "examples/retry_budget.rl",
        "retry budget runtime",
        "retry_budget_remaining",
        2,
    );
}

#[test]
fn rollout_handoff_example_executes() {
    assert_exported_i32_example(
        "examples/rollout_handoff.rl",
        "rollout handoff runtime",
        "rollout_handoff_score",
        63,
    );
}

#[test]
fn sensor_calibration_example_executes() {
    assert_exported_f64_example(
        "examples/sensor_calibration.rl",
        "sensor calibration runtime",
        "sensor_calibration_score",
        1.55,
    );
}

#[test]
fn calibration_pipeline_example_executes() {
    assert_exported_f64_example(
        "examples/calibration_pipeline.rl",
        "calibration pipeline runtime",
        "calibration_pipeline_score",
        23.25,
    );
}

#[test]
fn review_policy_factory_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/review_policy_factory.rl",
        "fun main: () -> ReviewDecision",
        "fun build_review_policy_factory: () -> ReviewDecision",
        r#"
export fun review_policy_factory_score: () -> Int32 = {
    val decision = () build_review_policy_factory
    val ReviewDecision { score, block_release, reviewer_lane } = decision
    score + reviewer_lane
}
"#,
    );

    let (mut store, instance) = instantiate_wat("review policy factory runtime", &wat);
    let review_policy_factory_score = instance
        .get_typed_func::<(), i32>(&store, "review_policy_factory_score")
        .expect("review policy factory wrapper should be host-callable");

    assert_eq!(
        review_policy_factory_score
            .call(&mut store, ())
            .expect("review policy factory wrapper should execute"),
        100
    );
}

#[test]
fn change_review_gate_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/change_review_gate.rl",
        "fun main: () -> ReviewPlan",
        "fun build_change_review_gate: () -> ReviewPlan",
        r#"
export fun change_review_gate_score: () -> Int32 = {
    val plan = () build_change_review_gate
    val ReviewPlan { change_id, score, escalation, accepted } = plan
    val escalation_score = escalation match {
        Some(id) => { id }
        None => { 0 }
    }
    val accepted_score = accepted match {
        true => { 1 }
        false => { 0 }
    }
    change_id + score + escalation_score + accepted_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("change review gate runtime", &wat);
    let change_review_gate_score = instance
        .get_typed_func::<(), i32>(&store, "change_review_gate_score")
        .expect("change review gate wrapper should be host-callable");

    assert_eq!(
        change_review_gate_score
            .call(&mut store, ())
            .expect("change review gate wrapper should execute"),
        255
    );
}

#[test]
fn service_monitor_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/service_monitor.rl",
        "fun main: () -> ServiceAlert",
        "fun build_service_monitor: () -> ServiceAlert",
        r#"
export fun service_monitor_score: () -> Int32 = {
    val alert = () build_service_monitor
    val ServiceAlert { severity, page, first_unowned, evidence, message } = alert
    val page_score = page match {
        true => { 1 }
        false => { 0 }
    }
    val owner_score = first_unowned match {
        Some(owner) => { owner }
        None => { 0 }
    }
    val evidence_score = (evidence, 0, |total, code| total + code) fold
    severity + page_score + owner_score + evidence_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("service monitor runtime", &wat);
    let service_monitor_score = instance
        .get_typed_func::<(), i32>(&store, "service_monitor_score")
        .expect("service monitor wrapper should be host-callable");

    assert_eq!(
        service_monitor_score
            .call(&mut store, ())
            .expect("service monitor wrapper should execute"),
        126
    );
}

#[test]
fn support_queue_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/support_queue.rl",
        "fun main: () -> QueueReport",
        "fun build_support_queue: () -> QueueReport",
        r#"
export fun support_queue_score: () -> Int32 = {
    val report = () build_support_queue
    val QueueReport {
        total_score,
        overdue_count,
        first_unowned,
        routed_count,
        routed_scores,
        escalation_codes
    } = report
    val unowned_score = first_unowned match {
        Some(id) => { id }
        None => { 0 }
    }
    val routed_score = (routed_scores, 0, |total, score| total + score) fold
    val escalation_score = (escalation_codes, 0, |total, code| total + code) fold
    total_score + overdue_count + unowned_score + routed_count + routed_score + escalation_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("support queue runtime", &wat);
    let support_queue_score = instance
        .get_typed_func::<(), i32>(&store, "support_queue_score")
        .expect("support queue wrapper should be host-callable");

    assert_eq!(
        support_queue_score
            .call(&mut store, ())
            .expect("support queue wrapper should execute"),
        1972
    );
}

#[test]
fn release_queue_snapshot_example_executes() {
    assert_exported_i32_example(
        "examples/release_queue_snapshot.rl",
        "release queue snapshot runtime",
        "release_queue_snapshot_score",
        27,
    );
}

#[test]
fn sprint_planner_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/sprint_planner.rl",
        "fun main: () -> SprintPlan",
        "fun build_sprint_planner: () -> SprintPlan",
        r#"
export fun sprint_planner_score: () -> Int32 = {
    val plan = () build_sprint_planner
    val SprintPlan {
        score,
        ready_effort,
        blocked_count,
        unowned_task,
        escalation_codes,
        candidate_scores
    } = plan
    val unowned_score = unowned_task match {
        Some(task) => { task }
        None => { 0 }
    }
    val escalation_score = (escalation_codes, 0, |total, code| total + code) fold
    val candidate_score = (candidate_scores, 0, |total, score| total + score) fold
    score + ready_effort + blocked_count + unowned_score + escalation_score + candidate_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("sprint planner runtime", &wat);
    let sprint_planner_score = instance
        .get_typed_func::<(), i32>(&store, "sprint_planner_score")
        .expect("sprint planner wrapper should be host-callable");

    assert_eq!(
        sprint_planner_score
            .call(&mut store, ())
            .expect("sprint planner wrapper should execute"),
        283
    );
}

#[test]
fn deploy_gate_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/deploy_gate.rl",
        "fun main: () -> GateReport",
        "fun build_deploy_gate: () -> GateReport",
        r#"
export fun deploy_gate_score: () -> Int32 = {
    val report = () build_deploy_gate
    val GateReport { release_score, blocker_codes, needs_review } = report
    val blocker_count = blocker_codes |> list_count
    val review_score = needs_review match {
        Some(score) => { score }
        None => { 0 }
    }
    release_score + blocker_count + review_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("deploy gate runtime", &wat);
    let deploy_gate_score = instance
        .get_typed_func::<(), i32>(&store, "deploy_gate_score")
        .expect("deploy gate wrapper should be host-callable");

    assert_eq!(
        deploy_gate_score
            .call(&mut store, ())
            .expect("deploy gate wrapper should execute"),
        140
    );
}

#[test]
fn fulfillment_batch_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/fulfillment_batch.rl",
        "fun main: () -> FulfillmentPlan",
        "fun build_fulfillment_batch: () -> FulfillmentPlan",
        r#"
export fun fulfillment_batch_score: () -> Int32 = {
    val plan = () build_fulfillment_batch
    val FulfillmentPlan {
        priority_total,
        fragile_count,
        first_unassigned,
        manual_count,
        manual_scores,
        audit_codes
    } = plan
    val unassigned_score = first_unassigned match {
        Some(id) => { id }
        None => { 0 }
    }
    val manual_score = (manual_scores, 0, |total, score| total + score) fold
    val audit_score = (audit_codes, 0, |total, code| total + code) fold
    priority_total + fragile_count + unassigned_score + manual_count + manual_score + audit_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("fulfillment batch runtime", &wat);
    let fulfillment_batch_score = instance
        .get_typed_func::<(), i32>(&store, "fulfillment_batch_score")
        .expect("fulfillment batch wrapper should be host-callable");

    assert_eq!(
        fulfillment_batch_score
            .call(&mut store, ())
            .expect("fulfillment batch wrapper should execute"),
        361
    );
}

#[test]
fn typed_impl_dispatch_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/typed_impl_dispatch.rl",
        "fun main: () -> DispatchDecision",
        "fun build_typed_impl_dispatch: () -> DispatchDecision",
        r#"
export fun typed_impl_dispatch_risk: () -> Float64 = {
    val decision = () build_typed_impl_dispatch
    val DispatchDecision { health_risk, rollout_risk, approved } = decision
    health_risk + rollout_risk
}
"#,
    );

    let (mut store, instance) = instantiate_wat("typed impl dispatch runtime", &wat);
    let typed_impl_dispatch_risk = instance
        .get_typed_func::<(), f64>(&store, "typed_impl_dispatch_risk")
        .expect("typed impl dispatch wrapper should be host-callable");

    let risk = typed_impl_dispatch_risk
        .call(&mut store, ())
        .expect("typed impl dispatch wrapper should execute");
    assert!(
        (risk - 69.5).abs() < 0.000_001,
        "typed impl dispatch risk should be 69.5, got {risk}"
    );
}

#[test]
fn generic_function_value_pipeline_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/generic_function_value_pipeline.rl",
        "fun main: () -> ScoreSummary",
        "fun build_generic_function_value_pipeline: () -> ScoreSummary",
        r#"
export fun generic_function_value_pipeline_score: () -> Float64 = {
    val summary = () build_generic_function_value_pipeline
    val ScoreSummary { normalized, first_score, adjusted_score } = summary
    first_score + adjusted_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("generic function value pipeline runtime", &wat);
    let generic_function_value_pipeline_score = instance
        .get_typed_func::<(), f64>(&store, "generic_function_value_pipeline_score")
        .expect("generic function value pipeline wrapper should be host-callable");

    let score = generic_function_value_pipeline_score
        .call(&mut store, ())
        .expect("generic function value pipeline wrapper should execute");
    assert!(
        (score - 3.25).abs() < 0.000_001,
        "generic function value pipeline score should be 3.25, got {score}"
    );
}

#[test]
fn experiment_scorecard_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/experiment_scorecard.rl",
        "fun main: () -> ExperimentDecision",
        "fun build_experiment_scorecard: () -> ExperimentDecision",
        r#"
export fun experiment_scorecard_score: () -> Float64 = {
    val decision = () build_experiment_scorecard
    val ExperimentDecision {
        preview_score,
        passing_total,
        accepted,
        fallback_bonus,
        normalized_scores
    } = decision
    val bonus_score = fallback_bonus match {
        Some(bonus) => { bonus }
        None => { 0.0 }
    }
    preview_score + passing_total + bonus_score
}
"#,
    );

    let (mut store, instance) = instantiate_wat("experiment scorecard runtime", &wat);
    let experiment_scorecard_score = instance
        .get_typed_func::<(), f64>(&store, "experiment_scorecard_score")
        .expect("experiment scorecard wrapper should be host-callable");

    let score = experiment_scorecard_score
        .call(&mut store, ())
        .expect("experiment scorecard wrapper should execute");
    assert!(
        (score - 29.3).abs() < 0.000_001,
        "experiment scorecard score should be 29.3, got {score}"
    );
}

#[test]
fn release_decision_engine_example_executes() {
    let wat = compile_example_with_rewritten_main_and_export_wrapper(
        "examples/release_decision_engine.rl",
        "fun main: () -> ReleaseDecision",
        "fun build_release_decision_engine: () -> ReleaseDecision",
        r#"
export fun release_decision_engine_score: () -> Float64 = {
    val decision = () build_release_decision_engine
    val ReleaseDecision {
        risk,
        blocked,
        primary_owner,
        first_blocker,
        audit_scores
    } = decision
    val audit_total = (audit_scores, 0.0, |total, score| total + score) fold
    risk + audit_total
}
"#,
    );

    let (mut store, instance) = instantiate_wat("release decision engine runtime", &wat);
    let release_decision_engine_score = instance
        .get_typed_func::<(), f64>(&store, "release_decision_engine_score")
        .expect("release decision engine wrapper should be host-callable");

    let score = release_decision_engine_score
        .call(&mut store, ())
        .expect("release decision engine wrapper should execute");
    assert!(
        (score - 38.3).abs() < 0.000_001,
        "release decision engine score should be 38.3, got {score}"
    );
}
