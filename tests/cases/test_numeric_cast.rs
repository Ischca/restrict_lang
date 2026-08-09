use restrict_lang::ast::{BinaryOp, ExprKind, TopDecl, Type};
use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn parse_source(source: &str) -> restrict_lang::ast::Program {
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(
        remaining.trim().is_empty(),
        "source should parse completely, remaining: {remaining:?}"
    );
    program
}

fn type_check_source(source: &str) -> Result<(), String> {
    let program = parse_source(source);
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|err| format!("Type error: {err}"))
}

fn compile_to_wat(source: &str) -> String {
    let program = parse_source(source);
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("source should type-check");

    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&program)
        .expect("source should generate WAT")
}

#[test]
fn parser_builds_cast_ast_for_parenthesized_binary() {
    let program = parse_source(
        r#"
fun main: () -> Float64 = {
    (1 + 2) as Float64
}
"#,
    );

    let TopDecl::Function(func) = &program.declarations[0] else {
        panic!("expected function declaration");
    };
    let ExprKind::Cast(cast) = &func
        .body
        .expr
        .as_deref()
        .expect("function body should return")
        .kind
    else {
        panic!("expected cast expression");
    };

    assert_eq!(cast.target, Type::Named("Float64".to_string()));
    let ExprKind::Binary(binary) = &cast.expr.kind else {
        panic!("expected cast operand to be binary");
    };
    assert_eq!(binary.op, BinaryOp::Add);
}

#[test]
fn numeric_casts_type_check_and_reject_non_numeric_sources() {
    type_check_source(
        r#"
fun ok: (x: Int32) -> Float64 = {
    x as Float64
}
"#,
    )
    .expect("numeric cast should type-check");

    let err = type_check_source(
        r#"
fun bad: (s: String) -> Float64 = {
    s as Float64
}
"#,
    )
    .expect_err("string to float cast should be rejected");
    assert!(
        err.contains("numeric cast"),
        "error should explain numeric cast restriction, got: {err}"
    );
}

#[test]
fn numeric_cast_codegen_emits_wasm_conversions() {
    let wat = compile_to_wat(
        r#"
export fun to_float: (x: Int32) -> Float64 = {
    x as Float64
}

export fun narrow: (x: Float64) -> Int32 = {
    x as Int32
}

export fun widen: (x: Int32) -> Int64 = {
    x as Int64
}
"#,
    );

    assert!(wat.contains("f64.convert_i32_s"), "{wat}");
    assert!(wat.contains("i32.trunc_f64_s"), "{wat}");
    assert!(wat.contains("i64.extend_i32_s"), "{wat}");

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("numeric cast WAT should parse: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| panic!("numeric cast Wasm should validate: {err}\n\n{wat}"));
}
