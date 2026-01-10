use restrict_lang::{parse_program, TypeChecker};

#[test]
fn test_sublifetime_constraint() {
    // Test basic sublifetime relationship
    let input = r#"
    record Database<~db> {
        id: Int32
    }
    
    record Transaction<~tx, ~db> where ~tx within ~db {
        db: Database<~db>
        txId: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~db> {
            val database = Database { id = 1 };
            with lifetime<~tx> where ~tx within ~db {
                val transaction = Transaction { db = database, txId = 100 };
                transaction.txId
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
fn test_invalid_sublifetime_constraint() {
    // Test invalid constraint - ~tx not within ~db
    let input = r#"
    record Database<~db> {
        id: Int32
    }
    
    record Transaction<~tx, ~db> where ~tx within ~db {
        db: Database<~db>
        txId: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~db> {
            val database = Database { id = 1 };
            Unit
        };
        
        // This should fail - ~tx is not within ~db scope
        with lifetime<~tx> {
            val transaction = Transaction { db = database, txId = 100 };
            Unit
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Expected type error for invalid sublifetime constraint"),
        Err(_) => {
            // Expected error
        }
    }
}

#[test]
fn test_temporal_borrowing() {
    // Test temporal borrowing pattern
    let input = r#"
    record Resource<~r> {
        data: Int32
    }
    
    fun borrowResource<~r> = resource: Resource<~r> -> Int32 {
        resource.data
    }
    
    fun main: () -> Int = {
        with lifetime<~r> {
            val resource = Resource { data = 42 };
            val result = (resource) borrowResource;
            result
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}

#[test]
fn test_transitive_sublifetime() {
    // Test transitive sublifetime relationships
    let input = r#"
    record Outer<~out> {
        id: Int32
    }
    
    record Middle<~mid, ~out> where ~mid within ~out {
        outer: Outer<~out>,
        midId: Int32
    }
    
    record Inner<~in, ~mid, ~out> where ~in within ~mid, ~mid within ~out {
        middle: Middle<~mid, ~out>,
        innerId: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~out> {
            val outer = Outer { id = 1 };
            with lifetime<~mid> where ~mid within ~out {
                val middle = Middle { outer = outer, midId = 10 };
                with lifetime<~in> where ~in within ~mid {
                    val inner = Inner { middle = middle, innerId = 100 };
                    inner.innerId
                }
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => {},
        Err(e) => panic!("Type checking failed: {:?}", e),
    }
}