use restrict_lang::{parse_program, TypeChecker};

/// 時間型(Temporal Types)の包括的テスト
/// Comprehensive Temporal Affine Types (TAT) tests

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_basic_temporal_scope() {
    // Test basic temporal scope management
    let input = r#"
    record File<~f> {
        path: String,
        content: String
    }
    
    fun main: () -> Int = {
        with lifetime<~f> {
            val file = File { path = "test.txt", content = "data" };
            file.content
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
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_nested_temporal_constraints() {
    // Test nested temporal constraints with 'within' relationships
    let input = r#"
    record Database<~db> {
        name: String,
        connection: String
    }
    
    record Transaction<~tx, ~db> where ~tx within ~db {
        id: Int32,
        db: Database<~db>
    }
    
    record Query<~q, ~tx, ~db> where ~q within ~tx, ~tx within ~db {
        sql: String,
        tx: Transaction<~tx, ~db>
    }
    
    fun main: () -> Int = {
        with lifetime<~db> {
            with lifetime<~tx> where ~tx within ~db {
                with lifetime<~q> where ~q within ~tx {
                    val db = Database { name = "mydb", connection = "localhost" };
                    val tx = Transaction { id = 1, db = db };
                    val query = Query { sql = "SELECT * FROM users", tx = tx };
                    query.sql
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

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_constraint_violation() {
    // Test that temporal constraint violations are caught
    let input = r#"
    record Transaction<~tx, ~db> where ~tx within ~db {
        id: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~tx> {
            with lifetime<~db> where ~db within ~tx {
                // This should fail: ~tx should be within ~db, not the other way around
                val tx = Transaction { id = 1 };
                tx.id
            }
        }
    }"#;
    
    let (_, program) = parse_program(input).unwrap();
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Ok(_) => panic!("Expected type checking to fail for constraint violation"),
        Err(e) => {
            assert!(e.to_string().contains("constraint") || e.to_string().contains("within"));
        },
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_scope_lifetime_ordering() {
    // Test that temporal scopes respect lifetime ordering
    let input = r#"
    record Resource<~r> {
        name: String,
        value: Int32
    }
    
    record Handle<~h, ~r> where ~h within ~r {
        resource: Resource<~r>
    }
    
    fun main: () -> Int = {
        with lifetime<~r> {
            val resource = Resource { name = "data", value = 42 };
            
            with lifetime<~h> where ~h within ~r {
                val handle = Handle { resource = resource };
                handle.resource.value
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
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_complex_temporal_relationships() {
    // Test complex temporal relationships with multiple constraints
    let input = r#"
    record System<~sys> {
        name: String
    }
    
    record Service<~svc, ~sys> where ~svc within ~sys {
        name: String,
        system: System<~sys>
    }
    
    record Connection<~conn, ~svc, ~sys> where ~conn within ~svc, ~svc within ~sys {
        id: Int32,
        service: Service<~svc, ~sys>
    }
    
    record Request<~req, ~conn, ~svc, ~sys> 
    where ~req within ~conn, ~conn within ~svc, ~svc within ~sys {
        data: String,
        connection: Connection<~conn, ~svc, ~sys>
    }
    
    fun main: () -> Int = {
        with lifetime<~sys> {
            with lifetime<~svc> where ~svc within ~sys {
                with lifetime<~conn> where ~conn within ~svc {
                    with lifetime<~req> where ~req within ~conn {
                        val system = System { name = "WebServer" };
                        val service = Service { name = "API", system = system };
                        val connection = Connection { id = 1, service = service };
                        val request = Request { data = "GET /users", connection = connection };
                        request.data
                    }
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

#[test]
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_type_with_generics() {
    // Test temporal types combined with generic types
    let input = r#"
    record Container<T, ~c> {
        value: T,
        created_at: String
    }
    
    record Manager<T, ~m, ~c> where ~m within ~c {
        container: Container<T, ~c>,
        status: String
    }
    
    fun main: () -> Int = {
        with lifetime<~c> {
            with lifetime<~m> where ~m within ~c {
                val container = Container { value = 42, created_at = "now" };
                val manager = Manager { container = container, status = "active" };
                manager.container.value
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
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_scope_isolation() {
    // Test that temporal scopes are properly isolated
    let input = r#"
    record Resource<~r> {
        id: Int32,
        name: String
    }
    
    fun main: () -> Int = {
        with lifetime<~r1> {
            val resource1 = Resource { id = 1, name = "First" };
            resource1.id
        };
        
        with lifetime<~r2> {
            val resource2 = Resource { id = 2, name = "Second" };
            resource2.id
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
#[ignore = "TAT (Temporal Affine Types) - deferred to v2.0"]
fn test_temporal_affine_usage() {
    // Test that temporal types follow affine usage rules
    let input = r#"
    record UniqueResource<~r> {
        token: String,
        value: Int32
    }
    
    fun main: () -> Int = {
        with lifetime<~r> {
            val resource = UniqueResource { token = "unique", value = 100 };
            // Using the resource once should be fine
            val result = resource.value;
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