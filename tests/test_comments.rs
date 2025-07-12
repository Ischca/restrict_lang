use restrict_lang::{parse_program, TypeChecker};

#[test]
fn test_single_line_comments() {
    let input = r#"
    // This is a comment
    val x = 42  // Another comment
    
    // Function with comments
    fun test = {
        // Comment inside function
        val y = 10;  // Inline comment
        y
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_multi_line_comments() {
    let input = r#"
    /* This is a
       multi-line comment */
    val x = 42
    
    /* Function with
       multi-line comments */
    fun test = {
        /* Comment
           inside function */
        val y = 10;
        y
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_mixed_comments() {
    let input = r#"
    // Single line comment
    /* Multi-line comment */
    val x = 42  // Inline comment
    
    /* Mixed
       comments */
    fun test = {
        // Single line
        val y = /* inline multi */ 10;
        y  /* trailing */
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_lambda_with_comments() {
    let input = r#"
    // Lambda with comments
    fun test = {
        // Create a lambda
        val add_one = |x| /* param x */ x + 1;  // adds one
        
        /* Apply the lambda */
        val result = (41) add_one;
        result
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_comments_between_tokens() {
    let input = r#"
    fun /* function */ test /* name */ = /* equals */ {
        val /* val keyword */ x /* var name */ = /* equals */ 42 /* value */;
        x /* return */
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}