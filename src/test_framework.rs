use crate::parser::parse_program;
use crate::type_checker::type_check;
use crate::codegen::generate;

pub struct TestCase {
    pub name: &'static str,
    pub input: &'static str,
    pub expected_ast: Option<&'static str>,
    pub expected_type_error: Option<&'static str>,
    pub expected_wat: Option<&'static str>,
}

pub fn run_test(test: &TestCase) -> Result<(), String> {
    println!("Running test: {}", test.name);
    
    // Parse
    let (remaining, ast) = parse_program(test.input)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input: '{}'", remaining));
    }
    
    if let Some(expected_ast) = test.expected_ast {
        let ast_str = format!("{:#?}", ast);
        if !ast_str.contains(expected_ast) {
            return Err(format!("AST mismatch.\nExpected to contain: {}\nGot: {}", 
                expected_ast, ast_str));
        }
    }
    
    // Type check
    match type_check(&ast) {
        Ok(()) => {
            if let Some(expected_error) = test.expected_type_error {
                return Err(format!("Expected type error '{}' but type check passed", 
                    expected_error));
            }
            
            // Code generation
            if let Some(expected_wat) = test.expected_wat {
                let wat = generate(&ast)
                    .map_err(|e| format!("Codegen error: {:?}", e))?;
                if !wat.contains(expected_wat) {
                    return Err(format!("WAT mismatch.\nExpected to contain: {}\nGot: {}", 
                        expected_wat, wat));
                }
            }
        }
        Err(e) => {
            if let Some(expected_error) = test.expected_type_error {
                let error_str = format!("{:?}", e);
                if !error_str.contains(expected_error) {
                    return Err(format!("Type error mismatch.\nExpected: {}\nGot: {}", 
                        expected_error, error_str));
                }
            } else {
                return Err(format!("Unexpected type error: {:?}", e));
            }
        }
    }
    
    Ok(())
}

#[macro_export]
macro_rules! test_suite {
    ($($test:expr),* $(,)?) => {
        #[cfg(test)]
        mod generated_tests {
            use super::*;
            use crate::test_framework::{TestCase, run_test};
            
            $(
                #[test]
                fn $test() {
                    if let Err(e) = run_test(&$test) {
                        panic!("{}", e);
                    }
                }
            )*
        }
    };
}