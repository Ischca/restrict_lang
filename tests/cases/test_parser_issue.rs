use restrict_lang::parse_program;

#[test]
fn test_exact_input_parsing() {
    let input = r#"record Point {
    x: Int32,
    y: Int32
}

fun main: () -> Int32 = {
    val p1 = Point { x: 10, y: 20 };
    val p2 = Point { x: 30, y: 40 };
    p1.x + p2.y
}"#;

    match parse_program(input) {
        Ok((rem, prog)) => {
            assert!(rem.trim().is_empty());
            assert_eq!(prog.declarations.len(), 2);
        }
        Err(e) => {
            panic!("Parse failed: {:?}", e);
        }
    }
}

#[test]
fn test_simple_parsing() {
    let input = r#"record Point {
    x: Int32,
    y: Int32
}

fun main: () -> Int32 = {
    42
}"#;

    match parse_program(input) {
        Ok((rem, prog)) => {
            assert!(rem.trim().is_empty());
            assert_eq!(prog.declarations.len(), 2);
        }
        Err(e) => {
            panic!("Parse failed: {:?}", e);
        }
    }
}
