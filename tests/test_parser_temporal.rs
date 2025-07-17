use restrict_lang::parser::parse_program;
use restrict_lang::ast::*;

#[test]
fn test_parse_record_with_temporal() {
    let input = r#"
    record File<~f> {
        handle: Int32
    }"#;
    
    let result = parse_program(input);
    assert!(result.is_ok());
    
    let (_, program) = result.unwrap();
    assert_eq!(program.declarations.len(), 1);
    
    match &program.declarations[0] {
        TopDecl::Record(rec) => {
            assert_eq!(rec.name, "File");
            assert_eq!(rec.type_params.len(), 1);
            assert_eq!(rec.type_params[0].name, "f");
            assert!(rec.type_params[0].is_temporal);
        }
        _ => panic!("Expected record declaration")
    }
}

#[test]
fn test_parse_record_with_temporal_constraint() {
    let input = r#"
    record Transaction<~tx, ~db> where ~tx within ~db {
        db: Database<~db>
        txId: Int32
    }"#;
    
    let result = parse_program(input);
    assert!(result.is_ok());
    
    let (_, program) = result.unwrap();
    assert_eq!(program.declarations.len(), 1);
    
    match &program.declarations[0] {
        TopDecl::Record(rec) => {
            assert_eq!(rec.name, "Transaction");
            assert_eq!(rec.type_params.len(), 2);
            assert!(rec.type_params[0].is_temporal);
            assert!(rec.type_params[1].is_temporal);
            assert_eq!(rec.type_params[0].name, "tx");
            assert_eq!(rec.type_params[1].name, "db");
            
            assert_eq!(rec.temporal_constraints.len(), 1);
            assert_eq!(rec.temporal_constraints[0].inner, "tx");
            assert_eq!(rec.temporal_constraints[0].outer, "db");
            
            // Check field type
            assert_eq!(rec.fields[0].name, "db");
            match &rec.fields[0].ty {
                Type::Temporal(name, temporals) => {
                    assert_eq!(name, "Database");
                    assert_eq!(temporals, &vec!["db".to_string()]);
                }
                _ => panic!("Expected temporal type")
            }
        }
        _ => panic!("Expected record declaration")
    }
}

#[test]
fn test_parse_function_with_temporal() {
    let input = r#"
    fun readFile<~io> = file: File<~io> {
        Unit
    }"#;
    
    let result = parse_program(input);
    assert!(result.is_ok());
    
    let (_, program) = result.unwrap();
    assert_eq!(program.declarations.len(), 1);
    
    match &program.declarations[0] {
        TopDecl::Function(fun) => {
            assert_eq!(fun.name, "readFile");
            assert_eq!(fun.type_params.len(), 1);
            assert!(fun.type_params[0].is_temporal);
            assert_eq!(fun.type_params[0].name, "io");
            
            // Check parameter type
            assert_eq!(fun.params[0].name, "file");
            match &fun.params[0].ty {
                Type::Temporal(name, temporals) => {
                    assert_eq!(name, "File");
                    assert_eq!(temporals, &vec!["io".to_string()]);
                }
                _ => panic!("Expected temporal type")
            }
        }
        _ => panic!("Expected function declaration")
    }
}

#[test]
fn test_parse_function_with_temporal_constraint() {
    let input = r#"
    fun beginTx<~db, ~tx> = db: Database<~db> 
    where ~tx within ~db {
        Unit
    }"#;
    
    let result = parse_program(input);
    assert!(result.is_ok());
    
    let (_, program) = result.unwrap();
    
    match &program.declarations[0] {
        TopDecl::Function(fun) => {
            assert_eq!(fun.name, "beginTx");
            assert_eq!(fun.type_params.len(), 2);
            assert!(fun.type_params[0].is_temporal);
            assert!(fun.type_params[1].is_temporal);
            
            assert_eq!(fun.temporal_constraints.len(), 1);
            assert_eq!(fun.temporal_constraints[0].inner, "tx");
            assert_eq!(fun.temporal_constraints[0].outer, "db");
        }
        _ => panic!("Expected function declaration")
    }
}

#[test]
fn test_parse_mixed_type_params() {
    // Test mixing regular type params with temporal ones
    let input = r#"
    fun process<T: Clone, ~res> = data: T resource: Resource<~res> {
        Unit
    }"#;
    
    let result = parse_program(input);
    assert!(result.is_ok());
    
    let (_, program) = result.unwrap();
    
    match &program.declarations[0] {
        TopDecl::Function(fun) => {
            assert_eq!(fun.type_params.len(), 2);
            
            // First param is regular with bounds
            assert!(!fun.type_params[0].is_temporal);
            assert_eq!(fun.type_params[0].name, "T");
            assert_eq!(fun.type_params[0].bounds.len(), 1);
            assert_eq!(fun.type_params[0].bounds[0].trait_name, "Clone");
            
            // Second param is temporal
            assert!(fun.type_params[1].is_temporal);
            assert_eq!(fun.type_params[1].name, "res");
        }
        _ => panic!("Expected function declaration")
    }
}