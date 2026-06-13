use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

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

#[test]
fn result_ok_and_err_type_check_with_expected_context() {
    let source = r#"
fun choose: (flag: Boolean) -> Result<Int32, Int32> = {
    flag then {
        Ok(1)
    } else {
        Err(2)
    }
}

fun main: () -> Int32 = {
    val result = true |> choose;

    result match {
        Ok(value) => {
            value
        }
        Err(code) => {
            code
        }
    }
}
"#;

    type_check_source(source).expect("Result constructors should use expected return type");
}

#[test]
fn ok_string_constructor_generates_valid_wat() {
    let source = r#"
fun main: () -> Result<String, Int32> = {
    Ok("ok")
}
"#;

    let wat = compile_to_wat(source).expect("Ok(String) should compile");
    assert!(wat.contains("ok"));

    let wasm = wat::parse_str(&wat)
        .unwrap_or_else(|err| panic!("Ok(String) WAT should parse: {err}\n\n{wat}"));
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("Ok(String) Wasm should validate: {err}\n\n{wat}"));
}

#[test]
fn result_constructor_works_through_generic_return_context() {
    let source = r#"
fun identity: <T>(value: T) -> T = {
    value
}

fun main: () -> Result<Int32, Int32> = {
    Ok(42) |> identity
}
"#;

    type_check_source(source).expect("generic expected return should infer Result");
}

#[test]
fn result_constructors_infer_from_generic_sibling_argument() {
    let source = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Result<Int32, Int32> = {
    (Ok(42), Err(7)) choose_first
}
"#;

    type_check_source(source).expect("Ok and Err should converge through generic constraints");
}

#[test]
fn result_constructors_infer_from_sibling_branches() {
    let source = r#"
fun main: (flag: Boolean) -> Result<Int32, String> = {
    val result = flag then {
        Ok(42)
    } else {
        Err("missing")
    }
    result
}
"#;

    type_check_source(source).expect("Ok and Err should infer through sibling branches");
}

#[test]
fn result_list_elements_infer_from_sibling_constructors() {
    let source = r#"
fun main: () -> List<Result<Int32, Int32>> = {
    [Ok(42), Err(7)]
}
"#;

    type_check_source(source).expect("Result list elements should infer from sibling constructors");
}

#[test]
fn result_requires_expected_type_for_err() {
    let source = r#"
fun main: () -> Int32 = {
    val result = Err("missing");
    0
}
"#;

    let err = type_check_source(source).expect_err("bare Err should be ambiguous");
    assert!(
        err.contains("Cannot infer type") && err.contains("result"),
        "error should explain the unresolved Result binding, got: {}",
        err
    );
}

#[test]
fn result_requires_expected_type_for_ok() {
    let source = r#"
fun main: () -> Int32 = {
    val result = Ok(42);
    0
}
"#;

    let err = type_check_source(source).expect_err("bare Ok should be ambiguous");
    assert!(
        err.contains("Cannot infer type") && err.contains("result"),
        "error should explain the unresolved Result binding, got: {}",
        err
    );
}

#[test]
fn result_local_constructors_infer_from_later_use_and_codegen() {
    let source = r#"
fun result_value_or_zero: (result: Result<Int32, Int32>) -> Int32 = {
    result match {
        Ok(value) => {
            value
        }
        Err(error) => {
            0
        }
    }
}

fun main: () -> Int32 = {
    val ok_result = Ok(41);
    val err_result = Err(5);
    (ok_result |> result_value_or_zero) + (err_result |> result_value_or_zero)
}
"#;

    let wat = compile_to_wat(source)
        .expect("local Result constructors should infer from later use and compile");

    assert!(wat.contains(";; Ok literal"));
    assert!(wat.contains(";; Err literal"));

    let wasm = wat::parse_str(&wat).expect("local Result inference should generate valid WAT");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("local Result inference should generate valid Wasm");
}

#[test]
fn result_local_list_payload_uses_later_match_context_for_codegen() {
    let source = r#"
fun first_i64: (values: List<Int64>) -> Int64 = {
    (values, 0) list_get
}

fun error_score: (message: String) -> Int64 = {
    0
}

fun main: () -> Int64 = {
    val result = Ok([1]);
    result match {
        Ok(values) => { values |> first_i64 }
        Err(message) => { message |> error_score }
    }
}
"#;

    let wat = compile_to_wat(source)
        .expect("local Result<List<T>, E> should compile with later match payload context");

    assert!(
        wat.contains("i64.store"),
        "Result<List<Int64>, String> payload should store list elements as i64:\n{wat}"
    );
    assert!(
        wat.contains("call $list_get_i64"),
        "later List<Int64> use should call the i64 list ABI:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).expect("local Result<List<Int64>> WAT should parse");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("local Result<List<Int64>> Wasm should validate");
}

#[test]
fn result_match_requires_ok_and_err_arms() {
    let source = r#"
fun main: () -> Int32 = {
    val result: Result<Int32, Int32> = Ok(42);

    result match {
        Ok(value) => {
            value
        }
    }
}
"#;

    let err = type_check_source(source).expect_err("Result match should be exhaustive");
    assert!(
        err.contains("Err"),
        "error should mention the missing Err arm, got: {}",
        err
    );
}

#[test]
fn result_validation_example_generates_valid_wat() {
    let source = include_str!("../examples/result_validation.rl");
    let wat = compile_to_wat(source).expect("Result example should compile to WAT");

    assert!(wat.contains(";; Ok literal"));
    assert!(wat.contains(";; Err literal"));

    let wasm = wat::parse_str(&wat).expect("Result example should generate valid WAT");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("Result example should generate valid Wasm");
}

#[test]
fn result_match_reuses_binding_name_with_different_payload_types() {
    let source = r#"
fun main: (flag: Boolean) -> Float64 = {
    val result: Result<Int32, Float64> = flag then {
        Ok(42)
    } else {
        Err(1.5)
    };

    result match {
        Ok(value) => {
            value as Float64
        }
        Err(value) => {
            value
        }
    }
}
"#;

    let wat = compile_to_wat(source)
        .expect("match arm bindings with the same source name should compile");

    assert!(
        wat.contains("__match_"),
        "conflicting arm bindings should use distinct emitted locals:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).expect("same-name Result match WAT should parse");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("same-name Result match Wasm should validate");
}

#[test]
fn result_match_conflict_locals_are_deterministic_across_compiles() {
    // Conflict-renamed pattern locals must not leak allocation addresses
    // into the WAT text: compiling the same source through two independent
    // parse/check/codegen runs must produce byte-identical output.
    let source = r#"
fun main: (flag: Boolean) -> Float64 = {
    val result: Result<Int32, Float64> = flag then {
        Ok(42)
    } else {
        Err(1.5)
    };

    result match {
        Ok(value) => {
            value as Float64
        }
        Err(value) => {
            value
        }
    }
}
"#;

    let first = compile_to_wat(source).expect("first compile should succeed");
    let second = compile_to_wat(source).expect("second compile should succeed");

    assert!(first.contains("__match_"));
    assert_eq!(
        first, second,
        "emitted WAT must be a deterministic function of the source"
    );
}
