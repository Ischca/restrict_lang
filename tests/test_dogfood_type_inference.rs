use restrict_lang::{check_v001_release_surface, parse_program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn parse_source(source: &str) -> Result<restrict_lang::Program, String> {
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

fn assert_valid_wat(source: &str) {
    let wat = compile_to_wat(source).expect("dogfood example should compile to WAT");
    assert!(
        wat.contains("(func $summarize_review"),
        "WAT should contain the review summarizer:\n{wat}"
    );
    assert!(
        wat.contains("(func $default_audit_sidecar"),
        "WAT should contain the expected-type sidecar example:\n{wat}"
    );
    assert!(
        wat.contains(";; map(list, mapper)"),
        "WAT should include list map lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; filter(list, predicate)"),
        "WAT should include list filter lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; fold(list, initial, reducer)"),
        "WAT should include list fold lowering:\n{wat}"
    );
    assert!(
        wat.contains(";; Ok literal") && wat.contains(";; Err literal"),
        "WAT should include Result constructor lowering:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("dogfood example generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("dogfood example generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn dogfood_type_inference_example_type_checks() {
    let source = include_str!("../examples/dogfood_type_inference.rl");

    type_check_source(source).expect("dogfood example should parse and type-check");
}

#[test]
fn dogfood_type_inference_example_generates_valid_wat() {
    let source = include_str!("../examples/dogfood_type_inference.rl");

    assert_valid_wat(source);
}

#[test]
fn dogfood_type_inference_example_executes() {
    let source = include_str!("../examples/dogfood_type_inference.rl").replace(
        "fun main: () -> ReviewSummary",
        "fun dogfood_review_summary: () -> ReviewSummary",
    ) + r#"

export fun dogfood_type_inference_score: () -> Int32 = {
    val ReviewSummary {
        owner,
        risk,
        approved,
        escalation
    } = () dogfood_review_summary;
    val approved_score = approved then {
        100
    } else {
        0
    };
    val escalation_score = escalation match {
        Ok(code) => {
            code
        }
        Err(code) => {
            0 - code
        }
    };

    owner + risk + approved_score + escalation_score
}
"#;
    let wat = compile_to_wat(&source)
        .expect("type inference dogfood runtime wrapper should compile to WAT");

    let (mut store, instance) = instantiate_wat("type inference dogfood runtime", &wat);
    let dogfood_type_inference_score = instance
        .get_typed_func::<(), i32>(&store, "dogfood_type_inference_score")
        .expect("runtime wrapper should be host-callable");

    assert_eq!(
        dogfood_type_inference_score
            .call(&mut store, ())
            .expect("runtime wrapper should execute"),
        205
    );
}

#[test]
fn dogfood_generic_export_gap_example_type_checks() {
    let source = include_str!("../examples/dogfood_generic_export_gap.rl");

    type_check_source(source).expect(
        "generic export gap dogfood should parse and type-check with built-in Option/Result",
    );
}

#[test]
fn dogfood_generic_export_gap_is_rejected_by_codegen() {
    let source = include_str!("../examples/dogfood_generic_export_gap.rl");

    let err = compile_to_wat(source)
        .expect_err("exported generic dogfood should still need a concrete ABI");
    assert!(
        err.contains("Exported generic function 'select_override' requires a concrete ABI"),
        "generic export gap should be visible as an explicit codegen rejection, got: {err}"
    );
}

#[test]
fn dogfood_generic_export_gap_is_rejected_by_release_surface() {
    let source = include_str!("../examples/dogfood_generic_export_gap.rl");
    let program = parse_source(source).expect("generic export gap dogfood should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic export gap dogfood should type-check before release validation");

    let err = check_v001_release_surface(&program, &checker)
        .expect_err("exported generic dogfood should fail the v0.0.1 release surface");
    let message = err.to_string();
    assert!(
        message.contains("Exported generic function 'select_override'"),
        "release surface should reject the generic export explicitly, got: {message}"
    );
}

#[test]
fn v001_exported_records_are_source_level_only_in_codegen() {
    let source = r#"
pub record ReleaseSlice {
    score: Int32
}

fun main: () -> Int32 = {
    1
}
"#;

    let wat = compile_to_wat(source)
        .expect("exported records should compile as source-level exports without a host ABI");
    assert!(
        wat.contains("source export record ReleaseSlice has no direct Wasm export"),
        "record export should be explicit as source-level metadata, got:\n{wat}"
    );
    assert!(
        !wat.contains("(export \"ReleaseSlice\""),
        "record export must not imply a host-visible Wasm ABI:\n{wat}"
    );
}

#[test]
fn v001_design_gap_user_defined_enum_declarations_are_still_not_parsed() {
    let source = r#"
enum ReviewState { Ready }

fun main: () -> Int32 = {
    0
}
"#;

    let err = parse_source(source).expect_err(
        "user-defined enum/ADT declarations are reserved but not implemented by the parser",
    );
    assert!(
        err.contains("enum declarations")
            && (err.contains("unsupported") || err.contains("not implemented")),
        "enum/ADT gap should be visible as an explicit unsupported-feature parse rejection, got: {err}"
    );
}
