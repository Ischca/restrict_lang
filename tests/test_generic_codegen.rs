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

fn assert_specialized_function_and_call(wat: &str, function_name: &str) {
    let function_decl = format!("(func ${function_name}");
    let function_call = format!("call ${function_name}");

    assert!(
        wat.contains(&function_decl),
        "expected specialized function `{function_name}` in WAT:\n{wat}"
    );
    assert!(
        wat.lines().map(str::trim).any(|line| line == function_call),
        "expected specialized call `{function_name}` in WAT:\n{wat}"
    );
}

fn assert_no_unspecialized_function_or_call(wat: &str, function_name: &str) {
    let function_decl = format!("(func ${function_name} ");
    let function_call = format!("call ${function_name}");

    assert!(
        !wat.contains(&function_decl),
        "generic function `{function_name}` must not be emitted with an unspecialized ABI:\n{wat}"
    );
    assert!(
        !wat.lines().map(str::trim).any(|line| line == function_call),
        "generic function `{function_name}` must not be called through an unspecialized ABI:\n{wat}"
    );
}

#[test]
fn user_defined_generic_float64_direct_call_uses_specialized_abi() {
    let source = r#"
fun keep_float: <T>(value: T) -> T = {
    value
}

fun main: () -> Float64 = {
    1.5 |> keep_float
}
"#;

    let wat = compile_to_wat(source).expect("Float64 generic direct call should compile");

    assert_specialized_function_and_call(&wat, "keep_float__Float64");
    assert_no_unspecialized_function_or_call(&wat, "keep_float");
}

#[test]
fn user_defined_generic_identity_compiles_to_wat() {
    let source = r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    42 |> identity_local
}
"#;

    let wat = compile_to_wat(source).expect("generic identity should compile");

    assert!(wat.contains("(func $identity_local"));
    assert!(wat.contains("call $identity_local"));
}

#[test]
fn user_defined_generic_multi_argument_call_compiles_to_wat() {
    let source = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val selected = (41, 0) choose_first;
    selected
}
"#;

    let wat = compile_to_wat(source).expect("generic multi-argument call should compile");

    assert!(wat.contains("(func $choose_first"));
    assert!(wat.contains("call $choose_first"));
}

#[test]
fn prelude_map_filter_pipeline_compiles_to_wat() {
    let source = include_str!("../examples/generic_inference.rl");

    let wat = compile_to_wat(source).expect("generic map/filter example should compile");

    assert!(wat.contains("$filter_loop"));
    assert!(wat.contains("$map_loop"));
    assert!(wat.contains("call_indirect"));
}

#[test]
fn option_map_compiles_to_wat() {
    let source = r#"
fun main: () -> Option<Int32> = {
    val maybe_value: Option<Int32> = Some(41);
    (maybe_value, |value| value + 1) map
}
"#;

    let wat = compile_to_wat(source).expect("Option map should compile");

    assert!(wat.contains(";; map(option, mapper)"));
    assert!(wat.contains("call_indirect"));
    assert!(wat.contains("Some tag"));
}

#[test]
fn option_filter_compiles_to_wat() {
    let source = r#"
fun main: () -> Option<Int32> = {
    val maybe_value: Option<Int32> = Some(41);
    (maybe_value, |value| value > 10) filter
}
"#;

    let wat = compile_to_wat(source).expect("Option filter should compile");

    assert!(wat.contains(";; filter(option, predicate)"));
    assert!(wat.contains("call_indirect"));
    assert!(wat.contains("None literal"));
}

#[test]
fn map_to_float_compiles_to_wat() {
    let source = r#"
fun main: () -> List<Float64> = {
    val numbers = [1, 2];
    (numbers, |value| 1.5) map
}
"#;

    let wat = compile_to_wat(source).expect("Float64 map result should compile");

    assert!(wat.contains("closure_call_1_i32_to_f64"));
    assert!(wat.contains("f64.store"));
}

#[test]
fn generic_record_float_field_access_generates_valid_wat() {
    let source = r#"
record Box<T> {
    value: T
}

fun main: () -> Float64 = {
    val box = Box { value: 1.5 };
    box.value
}
"#;

    let wat = compile_to_wat(source).expect("generic record Float64 field access should compile");

    assert!(
        wat.contains("f64.store"),
        "generic record literal should store Float64 field with f64.store:\n{wat}"
    );
    assert!(
        wat.contains("f64.load"),
        "generic record field access should load Float64 field with f64.load:\n{wat}"
    );
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("generic record field WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("generic record field Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn expected_generic_record_literal_with_empty_fields_generates_valid_wat() {
    let source = r#"
record Bundle<T> {
    selected: Option<T>,
    history: List<T>
}

fun main: () -> Bundle<Int32> = {
    Bundle {
        selected: None,
        history: []
    }
}
"#;

    let wat = compile_to_wat(source)
        .expect("expected return type should concrete generic record fields for codegen");

    assert!(
        wat.contains(";; None literal"),
        "expected generic record field should keep Option<Int32> lowering:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic record with empty fields WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic record with empty fields Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn unannotated_local_generic_record_literal_uses_block_return_expected_type() {
    let source = r#"
record Bundle<T> {
    selected: Option<T>,
    history: List<T>
}

fun main: () -> Bundle<Int32> = {
    val bundle = Bundle {
        selected: None,
        history: []
    };
    bundle
}
"#;

    let wat = compile_to_wat(source)
        .expect("unannotated local generic record literal should use block return expected type");

    assert!(
        wat.contains(";; None literal"),
        "local generic record field should lower None with the expected Option<Int32> type:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("local generic record with empty fields WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("local generic record with empty fields Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn unannotated_local_generic_record_literal_uses_field_access_context() {
    let source = r#"
record Box<T> {
    value: Option<T>
}

fun main: () -> Int32 = {
    val box = Box {
        value: None
    };
    box.value match {
        Some(value) => { value }
        None => { 42 }
    }
}
"#;

    let wat = compile_to_wat(source)
        .expect("generic record literal should use later field access and match context");

    assert!(
        wat.contains(";; None literal"),
        "local generic record field should lower None as Option<Int32>:\n{wat}"
    );
    assert!(
        wat.contains("i32.load"),
        "generic record field access should load the concrete Option<Int32> field:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("field-context generic record WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("field-context generic record Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn unannotated_local_generic_record_result_field_uses_match_context() {
    let source = r#"
record Box<T> {
    result: Result<T, Int32>
}

fun main: () -> Int32 = {
    val box = Box {
        result: Err(7)
    };
    box.result match {
        Ok(value) => { value }
        Err(code) => { 42 }
    }
}
"#;

    let wat =
        compile_to_wat(source).expect("generic record Result field should use later match context");

    assert!(
        wat.contains(";; Err literal"),
        "local generic record field should lower Err as Result<Int32, Int32>:\n{wat}"
    );
    assert!(
        wat.contains("i32.load"),
        "generic record Result field access should load concrete field storage:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("result-field generic record WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("result-field generic record Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn unannotated_local_generic_record_result_list_payload_uses_match_context() {
    let source = r#"
record Box<T> {
    result: Result<List<T>, String>
}

fun first_i64: (values: List<Int64>) -> Int64 = {
    (values, 0) list_get
}

fun main: () -> Int64 = {
    val box = Box {
        result: Ok([1])
    };
    box.result match {
        Ok(values) => { values |> first_i64 }
        Err(message) => { 0 }
    }
}
"#;

    let wat = compile_to_wat(source)
        .expect("nested Result<List<T>> record field should use later match payload context");

    assert!(
        wat.contains("i64.store"),
        "Result<List<Int64>> payload list should store i64 elements:\n{wat}"
    );
    assert!(
        wat.contains("call $list_get_i64"),
        "later List<Int64> use should call the i64 list ABI:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("nested result-list generic record WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("nested result-list generic record Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn generic_record_callable_field_uses_pipe_expected_list_context() {
    let source = r#"
record ListMapper<T> {
    f: List<T> -> List<T>
}

fun main: () -> List<Int32> = {
    val mapper = ListMapper {
        f: |values| values
    };
    val values = [];
    values |> (mapper.f)
}
"#;

    let wat = compile_to_wat(source)
        .expect("generic record callable field should infer List<Int32> from pipe context");

    assert!(
        wat.contains("call_indirect"),
        "generic record callable field should lower through the closure ABI:\n{wat}"
    );
    assert!(
        wat.contains("call_indirect (type $closure_call_1)"),
        "List<Int32> -> List<Int32> field should use the one-argument pointer ABI:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic record callable List field WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic record callable List field Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn generic_record_callable_field_uses_pipe_expected_option_context() {
    let source = r#"
record OptionMapper<T> {
    f: Option<T> -> Option<T>
}

fun main: () -> Option<Int32> = {
    val mapper = OptionMapper {
        f: |maybe| maybe
    };
    val maybe = None;
    maybe |> (mapper.f)
}
"#;

    let wat = compile_to_wat(source)
        .expect("generic record callable field should infer Option<Int32> from pipe context");

    assert!(
        wat.contains("call_indirect"),
        "generic record callable Option field should lower through the closure ABI:\n{wat}"
    );
    assert!(
        wat.contains("call_indirect (type $closure_call_1)"),
        "Option<Int32> -> Option<Int32> field should use the one-argument pointer ABI:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic record callable Option field WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic record callable Option field Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn generic_record_callable_field_uses_osv_call_argument_context() {
    let source = r#"
record Mapper<T> {
    f: T -> T
}

fun main: () -> Int32 = {
    val mapper = Mapper {
        f: |value| value + 1
    };
    (41) mapper.f
}
"#;

    let wat = compile_to_wat(source)
        .expect("generic record callable field should infer T from OSV call argument context");

    assert!(
        wat.contains("call_indirect (type $closure_call_1)"),
        "generic record callable field should lower as a one-argument closure call:\n{wat}"
    );
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic record callable OSV field WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic record callable OSV field Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn generic_record_option_callable_field_some_uses_match_context() {
    let source = r#"
record Mapper<T> {
    f: Option<T -> T>
}

fun main: () -> Int32 = {
    val mapper = Mapper {
        f: Some(|value| value + 1)
    };
    mapper.f match {
        Some(f) => { 41 |> f }
        None => { 0 }
    }
}
"#;

    let wat = compile_to_wat(source)
        .expect("generic Option callable field should infer T from match arm callable use");

    assert!(
        wat.contains(";; Some literal"),
        "generic Option callable field should lower Some closure payload:\n{wat}"
    );
    assert!(
        wat.contains("call_indirect (type $closure_call_1)"),
        "matched callable payload should lower as a one-argument closure call:\n{wat}"
    );
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic Option callable field Some WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic Option callable field Some Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn generic_record_option_callable_field_none_uses_match_context() {
    let source = r#"
record Mapper<T> {
    f: Option<T -> T>
}

fun main: () -> Int32 = {
    val mapper = Mapper {
        f: None
    };
    mapper.f match {
        Some(f) => { 41 |> f }
        None => { 0 }
    }
}
"#;

    let wat = compile_to_wat(source)
        .expect("generic None callable field should infer T from match arm callable use");

    assert!(
        wat.contains(";; None literal"),
        "generic Option callable field should lower None under Option<Int32 -> Int32>:\n{wat}"
    );
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic Option callable field None WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic Option callable field None Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn branch_expected_generic_record_literals_with_empty_fields_generate_valid_wat() {
    let source = r#"
record Bundle<T> {
    selected: Option<T>,
    history: List<T>
}

fun choose_bundle: (flag: Boolean) -> Bundle<Int32> = {
    flag then {
        Bundle {
            selected: None,
            history: []
        }
    } else {
        Bundle {
            selected: Some(41),
            history: [1, 2]
        }
    }
}

fun main: () -> Bundle<Int32> = {
    true |> choose_bundle
}
"#;

    let wat = compile_to_wat(source)
        .expect("expected return type should reach generic record literals in branches");

    assert!(
        wat.contains(";; None literal"),
        "then branch should lower None under the expected Bundle<Int32> type:\n{wat}"
    );
    assert!(
        wat.contains("i32.store"),
        "generic record branches should store concrete Int32-backed fields:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("branch generic record with empty fields WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("branch generic record with empty fields Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn float_list_filter_compiles_to_wat() {
    let source = r#"
fun main: () -> List<Float64> = {
    val readings = [1.5, 2.5];
    (readings, |value| value > 2.0) filter
}
"#;

    let wat = compile_to_wat(source).expect("Float64 list filter should compile");

    assert!(wat.contains("closure_call_1_f64_to_i32"));
    assert!(wat.contains("f64.load"));
    assert!(wat.contains("f64.store"));
}

#[test]
fn float_option_filter_compiles_to_wat() {
    let source = r#"
fun main: () -> Option<Float64> = {
    val reading: Option<Float64> = Some(1.5);
    (reading, |value| value > 1.0) filter
}
"#;

    let wat = compile_to_wat(source).expect("Float64 option filter should compile");

    assert!(wat.contains("closure_call_1_f64_to_i32"));
    assert!(wat.contains("f64.load"));
}

#[test]
fn float_option_map_compiles_to_wat() {
    let source = r#"
fun main: () -> Option<Float64> = {
    val reading: Option<Float64> = Some(1.5);
    (reading, |value| value + 1.0) map
}
"#;

    let wat = compile_to_wat(source).expect("Float64 option map should compile");

    assert!(wat.contains("closure_call_1_f64_to_f64"));
    assert!(wat.contains("f64.store"));
}

#[test]
fn float_fold_accumulator_compiles_to_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val numbers = [1, 2];
    (numbers, 0.0, |total, value| total + 1.0) fold
}
"#;

    let wat = compile_to_wat(source).expect("Float64 fold accumulator should compile");

    assert!(wat.contains("closure_call_2_f64_i32_to_f64"));
    assert!(wat.contains("local.set $iter_acc_f64"));
}

#[test]
fn int64_list_map_filter_fold_compile_to_wat() {
    let source = r#"
fun main: () -> Int64 = {
    val numbers: List<Int64> = [5000000000, 6000000000];
    val shifted = (numbers, |value| value + 10000000000) map;
    val kept = (shifted, |value| value > 12000000000) filter;
    val initial: Int64 = 0;
    (kept, initial, |total, value| total + value) fold
}
"#;

    let wat = compile_to_wat(source).expect("Int64 list iterators should compile");

    assert!(wat.contains("closure_call_1_i64_to_i64"));
    assert!(wat.contains("closure_call_1_i64_to_i32"));
    assert!(wat.contains("closure_call_2_i64_i64_to_i64"));
    assert!(wat.contains("local.set $iter_acc_i64"));
    assert!(wat.contains("i64.load"));
    assert!(wat.contains("i64.store"));

    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("Int64 list iterator WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("Int64 list iterator Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn int64_option_map_filter_compile_to_wat() {
    let source = r#"
fun main: () -> Option<Int64> = {
    val reading: Option<Int64> = Some(5000000000);
    val shifted = (reading, |value| value + 10000000000) map;
    (shifted, |value| value > 12000000000) filter
}
"#;

    let wat = compile_to_wat(source).expect("Int64 option iterators should compile");

    assert!(wat.contains("closure_call_1_i64_to_i64"));
    assert!(wat.contains("closure_call_1_i64_to_i32"));
    assert!(wat.contains("i64.load"));
    assert!(wat.contains("i64.store"));

    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("Int64 option iterator WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("Int64 option iterator Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn named_function_iterators_compile_to_wat() {
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

    let wat = compile_to_wat(source).expect("named function iterators should compile");

    assert!(wat.contains("(func $fnref_score_"));
    assert!(wat.contains("(func $fnref_keep_large_"));
    assert!(wat.contains("(func $fnref_add_score_"));
    assert!(wat.contains("closure_call_1_i32_to_f64"));
    assert!(wat.contains("closure_call_1_f64_to_i32"));
    assert!(wat.contains("closure_call_2_f64_f64_to_f64"));
    assert!(wat.contains("call $score"));
    assert!(wat.contains("call $keep_large"));
    assert!(wat.contains("call $add_score"));
}

#[test]
fn empty_local_list_infers_item_type_from_named_map_codegen_context() {
    let source = r#"
fun bump: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    val xs = [];
    val ys = (xs, bump) map;
    1
}
"#;

    let wat = compile_to_wat(source)
        .expect("empty local List should infer its source item type from map's mapper");

    assert!(wat.contains(";; map(list, mapper)"));
    assert!(wat.contains("call $bump"));
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("empty local list map WAT should parse: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("empty local list map Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn empty_local_option_infers_item_type_from_lambda_map_codegen_context() {
    let source = r#"
fun main: () -> Option<Int32> = {
    val maybe = None;
    (maybe, |value| value + 1) map
}
"#;

    let wat = compile_to_wat(source)
        .expect("empty local Option should infer its source item type from map lambda body");

    assert!(wat.contains(";; map(option, mapper)"));
    assert!(wat.contains("call_indirect"));
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("empty local option map WAT should parse: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("empty local option map Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn local_generic_function_mapper_compiles_to_wat() {
    let source = r#"
fun keep: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    val keep_plan = keep;
    (numbers, keep_plan) map
}
"#;

    let wat = compile_to_wat(source).expect("local generic mapper should compile");

    assert!(wat.contains("(func $keep__Int32"));
    assert!(wat.contains("(func $fnref_keep__Int32_"));
    assert!(wat.contains("call $keep__Int32"));
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("local generic mapper WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("local generic mapper Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn zero_arg_generic_function_value_uses_expected_return_codegen_context() {
    let source = r#"
fun empty_values: <T>() -> List<T> = {
    []
}

fun main: () -> List<Int32> = {
    val make_empty = empty_values;
    () make_empty
}
"#;

    let wat = compile_to_wat(source)
        .expect("zero-arg generic function value should specialize from expected return type");

    assert!(wat.contains("(func $empty_values__Int32"));
    assert!(wat.contains("fnref_empty_values__Int32_"));
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("zero-arg generic function value WAT should parse: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("zero-arg generic function value Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn local_generic_function_alias_chain_mapper_compiles_to_wat() {
    let source = r#"
fun keep: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    val keep_plan = keep;
    val mapper = keep_plan;
    (numbers, mapper) map
}
"#;

    let wat = compile_to_wat(source).expect("generic mapper alias chain should compile");

    assert!(wat.contains("(func $keep__Int32"));
    assert!(wat.contains("(func $fnref_keep__Int32_"));
    assert!(wat.contains("call $keep__Int32"));
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic mapper alias chain WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic mapper alias chain Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn local_generic_function_alias_chain_pipe_compiles_to_wat() {
    let source = r#"
fun keep: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val keep_plan = keep;
    val keep_again = keep_plan;
    41 |> keep_again
}
"#;

    let wat = compile_to_wat(source).expect("generic alias-chain pipe should compile");

    assert!(wat.contains("(func $keep__Int32"));
    assert!(wat.contains("(func $fnref_keep__Int32_"));
    assert!(wat.contains("call $keep__Int32"));
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("generic alias-chain pipe WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic alias-chain pipe Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn then_produced_lambda_alias_compiles_to_wat() {
    let source = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score + 1
    } else {
        |score| score * 2
    };
    41 |> adjust
}
"#;

    let wat = compile_to_wat(source).expect("then-produced lambda alias should compile");

    assert!(wat.contains("(if (result i32)"));
    assert!(wat.contains("call_indirect"));
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("then-produced lambda WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("then-produced lambda Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn then_produced_lambda_alias_with_prefix_bindings_compiles_to_wat() {
    let source = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val bonus = 1;
        |score| score + bonus
    } else {
        val factor = 2;
        |score| score * factor
    };
    41 |> adjust
}
"#;

    let wat = compile_to_wat(source)
        .expect("then-produced lambda alias with prefix bindings should compile");

    assert!(wat.contains("(if (result i32)"));
    assert!(wat.contains("call_indirect"));
    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("then-produced lambda with prefix WAT should parse: {err}\n\n{wat}")
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("then-produced lambda with prefix Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn match_produced_lambda_alias_with_pattern_capture_compiles_to_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val maybe_bonus: Option<Int32> = Some(2);
    val adjust = maybe_bonus match {
        Some(bonus) => { |score| score + bonus }
        None => { |score| score }
    };
    40 |> adjust
}
"#;

    let wat = compile_to_wat(source).expect("match-produced lambda alias should compile");

    assert!(wat.contains("call_indirect"));
    assert!(wat.contains("bonus_captured"));
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("match-produced lambda WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("match-produced lambda Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn immediate_match_produced_callable_pipe_compiles_to_wat() {
    let source = r#"
fun main: () -> Int32 = {
    3 |> (true match {
        true => { |value| value + 1 }
        false => { |value| value + 2 }
    })
}
"#;

    let wat =
        compile_to_wat(source).expect("immediate match-produced callable pipe should compile");

    assert!(wat.contains("(if (result i32)"));
    assert!(wat.contains("call_indirect"));
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("immediate match callable WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("immediate match callable Wasm should validate: {err}\n\n{wat}")
        });
}

#[test]
fn option_match_produced_callable_pipe_declares_pattern_locals() {
    let source = r#"
fun main: () -> Int32 = {
    val maybe_mapper: Option<Int32 -> Int32> = None;
    3 |> (maybe_mapper match {
        Some(mapper) => { mapper }
        None => { |value| value + 2 }
    })
}
"#;

    let wat =
        compile_to_wat(source).expect("match-produced callable pipe should declare pattern locals");

    assert!(wat.contains("mapper"));
    assert!(wat.contains("call_indirect"));
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("option match callable WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("option match callable Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn then_produced_mapper_alias_compiles_to_wat() {
    let source = r#"
fun main: (flag: Boolean) -> List<Int32> = {
    val numbers = [1, 2, 3];
    val mapper = flag then {
        |score| score + 1
    } else {
        |score| score * 2
    };
    (numbers, mapper) map
}
"#;

    let wat = compile_to_wat(source).expect("then-produced mapper alias should compile");

    assert!(wat.contains("$map_loop"));
    assert!(wat.contains("call_indirect"));
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("then-produced mapper WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("then-produced mapper Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn prelude_identity_alias_compiles_to_wat() {
    let source = r#"
fun main: () -> Float64 = {
    val keep = identity;
    1.5 |> keep
}
"#;

    let wat = compile_to_wat(source).expect("identity alias should compile");

    assert!(
        wat.contains("fnref_identity_"),
        "identity alias should lower to a typed identity function reference:\n{wat}"
    );
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("identity alias WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("identity alias Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn prelude_map_alias_compiles_to_wat() {
    let source = r#"
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    val apply_map = map;
    (numbers, |value| value + 1) apply_map
}
"#;

    let wat = compile_to_wat(source).expect("map alias should compile");

    assert!(
        wat.contains("$map_loop"),
        "map alias should use the same inline map lowering:\n{wat}"
    );
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("map alias WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("map alias Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn prelude_filter_and_fold_aliases_compile_to_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val numbers = [1, 2, 3, 4];
    val apply_filter = filter;
    val apply_fold = fold;
    val kept = (numbers, |value| value > 1) apply_filter;
    (kept, 0, |total, value| total + value) apply_fold
}
"#;

    let wat = compile_to_wat(source).expect("filter/fold aliases should compile");

    assert!(
        wat.contains("$filter_loop"),
        "filter alias should use the same inline filter lowering:\n{wat}"
    );
    assert!(
        wat.contains("$fold_loop"),
        "fold alias should use the same inline fold lowering:\n{wat}"
    );
    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("filter/fold alias WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("filter/fold alias Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn prelude_fold_compiles_to_wat() {
    let source = r#"
fun main: () -> Int32 = {
    val numbers = [1, 2, 3];
    (numbers, 0, |acc, n| acc + n) fold
}
"#;

    let wat = compile_to_wat(source).expect("generic fold should compile");

    assert!(wat.contains("$fold_loop"));
    assert!(wat.contains("call_indirect"));
}

#[test]
fn record_destructuring_with_prelude_pipeline_compiles_to_wat() {
    let source = include_str!("../examples/deploy_gate.rl");

    let wat = compile_to_wat(source).expect("deploy gate example should compile");

    assert!(wat.contains("(func $gate_pull_request"));
    assert!(wat.contains("$filter_loop"));
    assert!(wat.contains("$map_loop"));
}

#[test]
fn release_readiness_example_compiles_captured_lambdas_to_wat() {
    let source = include_str!("../examples/release_readiness.rl");

    let wat = compile_to_wat(source).expect("release readiness example should compile");

    assert!(wat.contains("(func $assess_release"));
    assert!(wat.contains("$fold_loop"));
    assert!(wat.contains("$visibility_penalty_captured"));
    assert!(wat.contains("call_indirect"));
}
