use restrict_lang::{parse_program, TypeChecker, generate};

// シンプルなコンパイル関数
fn compile_simple(source: &str) -> Result<String, String> {
    // 最初と最後の空白を削除
    let trimmed = source.trim();
    
    println!("Compiling: {:?}", trimmed);
    
    let (remaining, ast) = parse_program(trimmed)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Check if we parsed any declarations
    if ast.declarations.is_empty() && !trimmed.trim().is_empty() {
        return Err(format!("No declarations parsed from non-empty input"));
    }
        
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input: {:?}", remaining));
    }
    
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_empty_list_pattern() {
    let src = r#"
fun main: () -> Int = {
    with Arena {
        val lst = []
        val result = (lst) match {
            [] => { 42 }
            _ => { 0 }
        }
        result
    }
}
"#;
    
    let wat = compile_simple(src).unwrap();
    assert!(wat.contains("i32.load"));
}

#[test]
#[ignore = "Cons pattern [head | tail] parsing issue"]
fn test_cons_pattern() {
    let src = r#"
fun main: () -> Int = {
    with Arena {
        val lst = [10, 20, 30];
        val result = lst match {
            [] => { 0 }
            [head | tail] => { head }
            _ => { -1 }
        };
        result
    }
}
"#;
    
    let wat = compile_simple(src).unwrap();
    assert!(wat.contains("i32.gt_s"));
}

#[test]
#[ignore = "Cons pattern [head | tail] parsing issue"]
fn test_exact_pattern() {
    let src = r#"
fun main: () -> Int = {
    with Arena {
        val lst = [1, 2, 3];
        val result = lst match {
            [] => { 0 }
            [a] => { a }
            [a, b] => { a + b }
            [a, b, c] => { a + b + c }
            _ => { -1 }
        };
        result
    }
}
"#;
    
    let wat = compile_simple(src).unwrap();
    assert!(wat.contains("i32.const 3"));
}