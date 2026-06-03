#[test]
fn test_debug_pattern() {
    use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

    let source = r#"
        fun test_option: (opt: Option<Int32>) -> Int32 = {
            opt match {
                Some(n) => { n }
                None => { 0 }
            }
        }

        fun main: () -> Int32 = {
            val some_score = Some(42) |> test_option
            val none_val: Option<Int32> = None
            val none_score = none_val |> test_option
            some_score + none_score
        }
    "#;

    let (remaining, ast) = parse_program(source).expect("current Option syntax should parse");
    assert!(remaining.trim().is_empty());

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .expect("current Option syntax should type-check");

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&ast)
        .expect("current Option syntax should generate WAT");
    assert!(wat.contains("(func $test_option"));
}
