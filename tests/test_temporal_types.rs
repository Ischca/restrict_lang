use restrict_lang::{parse_program, TypeChecker};

#[test]
fn test_temporal_type_basic() {
    // Basic temporal type variable
    let input = r#"
    record File<~f> {
        handle: Int32  // Simplified for now
    }
    
    fun readFile<~io> = file: File<~io> {
        42  // Dummy return
    }
    
    fun main = {
        val file = File { handle: 1 };
        (file) readFile
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_temporal_constraint_within() {
    // Test ~tx within ~db constraint
    let input = r#"
    record Database<~db> {
        id: Int32
    }
    
    record Transaction<~tx, ~db> where ~tx within ~db {
        db: Database<~db>
        txId: Int32
    }
    
    fun beginTx<~db, ~tx> = db: Database<~db> -> Transaction<~tx, ~db>
    where ~tx within ~db {
        Transaction { db: db, txId: 1 }
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_temporal_inference() {
    // Temporal variables should be inferred
    let input = r#"
    record Resource<~r> {
        id: Int32
    }
    
    fun useResource<~r> = res: Resource<~r> {
        res.id
    }
    
    fun main = {
        val res = Resource { id: 42 };  // ~r should be inferred
        (res) useResource
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
fn test_temporal_escape_error() {
    // Should error when trying to return temporal outside its scope
    let input = r#"record File<~f> {
    handle: Int32
}

fun leakFile<~io> = {
    val file = File { handle = 1 };  // file: File<~io>
    file  // ERROR: Cannot return File<~io> outside ~io
}

fun main = {
    Unit
}"#;
    
    let (remaining, program) = parse_program(input).unwrap();
    
    // Debug: Check if all declarations were parsed
    if !remaining.trim().is_empty() {
        panic!("Parser failed to parse all declarations. Remaining: {:?}", remaining);
    }
    
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Expected type error, but checking succeeded"),
        Err(e) => println!("Got expected error: {:?}", e),
    }
}

#[test]
fn test_temporal_with_context() {
    // Context creates temporal scope
    let input = r#"
    context FileSystem<~fs> {
        open: String -> File<~fs>
    }
    
    record File<~f> {
        handle: Int32
    }
    
    fun main = {
        with FileSystem {
            val file = open("test.txt");  // file: File<~fs>
            file.handle
        }  // file cleaned up here
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}

#[test]
fn test_nested_temporal_scopes() {
    // Nested temporal relationships
    let input = r#"
    record Outer<~out> {
        id: Int32
    }
    
    record Inner<~in, ~out> where ~in within ~out {
        outer: Outer<~out>
        data: Int32
    }
    
    fun nested<~a, ~b> = outer: Outer<~a> -> Inner<~b, ~a>
    where ~b within ~a {
        Inner { outer: outer, data: 42 }
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    assert!(checker.check_program(&program).is_ok());
}