use restrict_lang::{parse_program, TypeChecker};

#[test]
fn test_basic_method_call() {
    let input = r#"
record Point {
    x: Int32,
    y: Int32
}

impl Point {
    fn distance(self, other: Point) -> Float64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy) |> sqrt
    }
}

fn main() {
    let p1 = Point { x: 0, y: 0 };
    let p2 = Point { x: 3, y: 4 };
    let dist = p1.distance(p2);
    dist
}
"#;

    match parse_program(input) {
        Ok((remaining, program)) => {
            assert!(remaining.trim().is_empty(), "Should parse all input");
            
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&program) {
                Ok(_) => {
                    // Test passed - method resolution should work
                    println!("Method resolution test passed");
                }
                Err(e) => {
                    panic!("Type checking failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_method_with_no_args() {
    let input = r#"
record Counter {
    count: Int32
}

impl Counter {
    fn get(self) -> Int32 {
        self.count
    }
    
    fn increment(self) -> Counter {
        Counter { count: self.count + 1 }
    }
}

fn main() {
    let counter = Counter { count: 5 };
    let value = counter.get();
    let next = counter.increment();
    value
}
"#;

    match parse_program(input) {
        Ok((remaining, program)) => {
            assert!(remaining.trim().is_empty(), "Should parse all input");
            
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&program) {
                Ok(_) => {
                    println!("No-arg method test passed");
                }
                Err(e) => {
                    panic!("Type checking failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_method_call_type_mismatch() {
    let input = r#"
record Calculator {
    value: Int32
}

impl Calculator {
    fn add(self, other: Int32) -> Int32 {
        self.value + other
    }
}

fn main() {
    let calc = Calculator { value: 10 };
    let result = calc.add("not a number");  // Should fail - wrong argument type
    result
}
"#;

    match parse_program(input) {
        Ok((_, program)) => {
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&program) {
                Ok(_) => {
                    panic!("Type checking should have failed due to argument type mismatch");
                }
                Err(e) => {
                    println!("Expected type error: {:?}", e);
                    // Should get a type mismatch error
                }
            }
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_method_call_arity_mismatch() {
    let input = r#"
record Adder {
    base: Int32
}

impl Adder {
    fn add(self, a: Int32, b: Int32) -> Int32 {
        self.base + a + b
    }
}

fn main() {
    let adder = Adder { base: 1 };
    let result = adder.add(2);  // Should fail - missing second argument
    result
}
"#;

    match parse_program(input) {
        Ok((_, program)) => {
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&program) {
                Ok(_) => {
                    panic!("Type checking should have failed due to arity mismatch");
                }
                Err(e) => {
                    println!("Expected arity error: {:?}", e);
                    // Should get an arity mismatch error
                }
            }
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_undefined_method() {
    let input = r#"
record Empty {
    value: Int32
}

fn main() {
    let obj = Empty { value: 42 };
    let result = obj.nonexistent();  // Should fail - method doesn't exist
    result
}
"#;

    match parse_program(input) {
        Ok((_, program)) => {
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&program) {
                Ok(_) => {
                    panic!("Type checking should have failed due to undefined method");
                }
                Err(e) => {
                    println!("Expected undefined method error: {:?}", e);
                    // Should get an undefined method error
                }
            }
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_field_access_vs_method_call() {
    let input = r#"
record Data {
    value: Int32
}

impl Data {
    fn getValue(self) -> Int32 {
        self.value
    }
}

fn main() {
    let data = Data { value: 42 };
    let field_access = data.value;    // Field access
    let method_call = data.getValue(); // Method call
    field_access + method_call
}
"#;

    match parse_program(input) {
        Ok((remaining, program)) => {
            assert!(remaining.trim().is_empty(), "Should parse all input");
            
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&program) {
                Ok(_) => {
                    println!("Field access vs method call test passed");
                }
                Err(e) => {
                    panic!("Type checking failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}

#[test]
fn test_chained_method_calls() {
    let input = r#"
record Builder {
    value: Int32
}

impl Builder {
    fn add(self, n: Int32) -> Builder {
        Builder { value: self.value + n }
    }
    
    fn multiply(self, n: Int32) -> Builder {
        Builder { value: self.value * n }
    }
    
    fn build(self) -> Int32 {
        self.value
    }
}

fn main() {
    let builder = Builder { value: 1 };
    let result = builder.add(5).multiply(2).build();  // Should work: 1+5=6, 6*2=12
    result
}
"#;

    match parse_program(input) {
        Ok((remaining, program)) => {
            assert!(remaining.trim().is_empty(), "Should parse all input");
            
            let mut type_checker = TypeChecker::new();
            match type_checker.check_program(&program) {
                Ok(_) => {
                    println!("Chained method calls test passed");
                }
                Err(e) => {
                    panic!("Type checking failed: {:?}", e);
                }
            }
        }
        Err(e) => {
            panic!("Parse error: {:?}", e);
        }
    }
}