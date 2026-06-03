use restrict_lang::{parse_program, Program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn parse_source(source: &str) -> Result<Program, String> {
    let (remaining, program) = parse_program(source).map_err(|e| format!("Parse error: {e:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    Ok(program)
}

fn type_check_source(source: &str) -> Result<(), String> {
    let program = parse_source(source)?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {e}"))
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

#[test]
fn dogfood_support_escalation_example_type_checks() {
    let source = include_str!("../examples/dogfood_support_escalation_inference.rl");

    type_check_source(source)
        .expect("support escalation dogfood example should parse and type-check");
}

#[test]
fn dogfood_support_escalation_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_support_escalation_inference.rl");
    let wat =
        compile_to_wat(source).expect("support escalation dogfood example should compile to WAT");

    assert!(
        wat.contains("(func $plan_support_escalation"),
        "WAT should contain the support escalation planner:\n{wat}"
    );
    assert!(
        wat.contains("(func $move_ticket_stream"),
        "WAT should contain the explicit affine list move helper:\n{wat}"
    );
    assert!(
        wat.contains("(func $move_ticket"),
        "WAT should contain the explicit affine record move helper:\n{wat}"
    );
    assert!(
        wat.contains(";; filter(list, predicate)"),
        "WAT should include list filter lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; map(list, mapper)"),
        "WAT should include list map lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; fold(list, initial, reducer)"),
        "WAT should include list fold lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; filter(option, predicate)") && wat.contains(";; map(option, mapper)"),
        "WAT should include Option filter and map lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; Ok literal") && wat.contains(";; Err literal"),
        "WAT should include Result constructor lowering:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("support escalation example generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("support escalation example generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_support_escalation_example_executes() {
    let dogfood_source = include_str!("../examples/dogfood_support_escalation_inference.rl")
        .replace(
            "fun main: () -> EscalationPlan",
            "fun build_support_escalation_plan: () -> EscalationPlan",
        );
    let source = format!(
        "{}\n{}",
        dogfood_source,
        r#"
export fun support_escalation_score: () -> Int32 = {
    val plan = () build_support_escalation_plan;
    val audit_tags = [5, 9];
    val audit_tag_score = audit_tags |> sum_tags;
    val audit_score = (4, 90, 60, 1, 1, audit_tag_score, 1) score_fields;
    val candidate_tags = [10, 4];
    val candidate_tag_score = candidate_tags |> sum_tags;
    val candidate_score = (5, 120, 60, 1, 1, candidate_tag_score, 2) score_fields;

    audit_score + candidate_score
}
"#
    );
    let wat = compile_to_wat(&source)
        .expect("support escalation dogfood runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("support escalation dogfood runtime", &wat);
    let support_escalation_score = instance
        .get_typed_func::<(), i32>(&store, "support_escalation_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        support_escalation_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        303
    );
}
