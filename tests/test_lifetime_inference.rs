use restrict_lang::{parse_program, TypeChecker};
use restrict_lang::lifetime_inference::LifetimeInference;

#[test]
fn test_basic_lifetime_inference() {
    // Test that lifetime inference runs without errors
    let input = r#"
    record File<~f> {
        handle: Int32
    }
    
    fun processFile<~io> = file: File<~io> {
        file.handle
    }
    
    fun main = {
        42
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    
    // Test lifetime inference directly
    let mut inference = LifetimeInference::new();
    let result = inference.infer_program(&program);
    assert!(result.is_ok(), "Lifetime inference should succeed");
    
    // Test through type checker
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
fn test_lifetime_inference_with_constraints() {
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
        Transaction { db = db, txId = 1 }
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    
    let mut inference = LifetimeInference::new();
    let result = inference.infer_program(&program);
    assert!(result.is_ok(), "Lifetime inference should handle constraints");
}

#[test]
fn test_lifetime_inference_escape_detection() {
    // This should eventually fail during inference
    let input = r#"
    record File<~f> {
        handle: Int32
    }
    
    fun leakFile<~io> = {
        val file = File { handle = 1 };
        file  // Trying to return temporal value
    }
    "#;
    
    let (_, program) = parse_program(input).unwrap();
    
    let mut inference = LifetimeInference::new();
    // For now, inference just collects info, actual validation happens in type checker
    let result = inference.infer_program(&program);
    assert!(result.is_ok(), "Inference collection phase should succeed");
}