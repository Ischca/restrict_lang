use restrict_lang::{parse_program, TypeChecker};

fn type_check_program(source: &str) -> Result<(), String> {
    // Parse
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;

    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input: '{}'", remaining));
    }

    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))
}

#[test]
#[ignore = "Uses non-EBNF v1.0 syntax"]
fn test_list_pattern_parse() {
    let input = r#"fun test: () -> Int32 = {
    val lst: List<Int32> = [];
    val result = lst match {
        [] => { 42 }
        _ => { 0 }
    };
    result
}"#;

    type_check_program(input).expect("list pattern should type check");
}
