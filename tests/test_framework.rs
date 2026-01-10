/// Comprehensive Test Framework for Restrict Language
/// 
/// This module provides a structured approach to testing all aspects of the language:
/// - Syntax validation
/// - Type system correctness
/// - Code generation accuracy
/// - Runtime behavior
/// - Performance characteristics

use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub description: String,
    pub input: String,
    pub expected: TestExpectation,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum TestExpectation {
    /// Test should parse successfully
    ParseSuccess,
    /// Test should fail parsing with specific error
    ParseError(String),
    /// Test should type check successfully
    TypeCheckSuccess,
    /// Test should fail type checking with specific error
    TypeCheckError(String),
    /// Test should generate specific WAT code
    CodeGenContains(Vec<String>),
    /// Test should run and produce specific output
    RuntimeOutput(String),
}

pub struct TestRunner {
    tests: Vec<TestCase>,
    results: HashMap<String, TestResult>,
}

#[derive(Debug)]
pub struct TestResult {
    pub passed: bool,
    pub message: String,
    pub duration: std::time::Duration,
}

impl TestRunner {
    pub fn new() -> Self {
        TestRunner {
            tests: Vec::new(),
            results: HashMap::new(),
        }
    }
    
    pub fn add_test(&mut self, test: TestCase) {
        self.tests.push(test);
    }
    
    pub fn run_all(&mut self) -> TestSummary {
        let mut summary = TestSummary::default();
        
        for test in &self.tests {
            let start = std::time::Instant::now();
            let result = self.run_single_test(test);
            let duration = start.elapsed();
            
            if result.passed {
                summary.passed += 1;
            } else {
                summary.failed += 1;
                summary.failed_tests.push(test.name.clone());
            }
            
            self.results.insert(test.name.clone(), TestResult {
                passed: result.passed,
                message: result.message,
                duration,
            });
        }
        
        summary.total = self.tests.len();
        summary
    }
    
    pub fn run_by_tag(&mut self, tag: &str) -> TestSummary {
        let mut summary = TestSummary::default();
        
        for test in &self.tests {
            if test.tags.contains(&tag.to_string()) {
                let start = std::time::Instant::now();
                let result = self.run_single_test(test);
                let duration = start.elapsed();
                
                if result.passed {
                    summary.passed += 1;
                } else {
                    summary.failed += 1;
                    summary.failed_tests.push(test.name.clone());
                }
                
                self.results.insert(test.name.clone(), TestResult {
                    passed: result.passed,
                    message: result.message,
                    duration,
                });
            }
        }
        
        summary.total = summary.passed + summary.failed;
        summary
    }
    
    fn run_single_test(&self, test: &TestCase) -> TestResult {
        match &test.expected {
            TestExpectation::ParseSuccess => self.test_parse_success(&test.input),
            TestExpectation::ParseError(expected_error) => {
                self.test_parse_error(&test.input, expected_error)
            }
            TestExpectation::TypeCheckSuccess => self.test_type_check_success(&test.input),
            TestExpectation::TypeCheckError(expected_error) => {
                self.test_type_check_error(&test.input, expected_error)
            }
            TestExpectation::CodeGenContains(patterns) => {
                self.test_codegen_contains(&test.input, patterns)
            }
            TestExpectation::RuntimeOutput(_output) => {
                // Runtime testing would require WASM execution
                TestResult {
                    duration: std::time::Duration::ZERO,
                    passed: false,
                    message: "Runtime testing not yet implemented".to_string(),
                }
            }
        }
    }
    
    fn test_parse_success(&self, input: &str) -> TestResult {
        match parse_program(input) {
            Ok((remaining, _program)) => {
                if remaining.trim().is_empty() {
                    TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: true,
                        message: "Parsed successfully".to_string(),
                    }
                } else {
                    TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!("Unparsed input remains: {}", remaining),
                    }
                }
            }
            Err(e) => TestResult {
                    duration: std::time::Duration::ZERO,
                passed: false,
                message: format!("Parse error: {:?}", e),
            },
        }
    }
    
    fn test_parse_error(&self, input: &str, expected_error: &str) -> TestResult {
        match parse_program(input) {
            Ok((_, _)) => TestResult {
                    duration: std::time::Duration::ZERO,
                passed: false,
                message: "Expected parse error but parsing succeeded".to_string(),
            },
            Err(e) => {
                let error_msg = format!("{:?}", e);
                if error_msg.contains(expected_error) {
                    TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: true,
                        message: format!("Got expected parse error: {}", error_msg),
                    }
                } else {
                    TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!(
                            "Got parse error but didn't match expected. Got: {}, Expected to contain: {}",
                            error_msg, expected_error
                        ),
                    }
                }
            }
        }
    }
    
    fn test_type_check_success(&self, input: &str) -> TestResult {
        match parse_program(input) {
            Ok((remaining, program)) => {
                if !remaining.trim().is_empty() {
                    return TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!("Unparsed input remains: {}", remaining),
                    };
                }
                
                let mut checker = TypeChecker::new();
                match checker.check_program(&program) {
                    Ok(()) => TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: true,
                        message: "Type checked successfully".to_string(),
                    },
                    Err(e) => TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!("Type check error: {:?}", e),
                    },
                }
            }
            Err(e) => TestResult {
                    duration: std::time::Duration::ZERO,
                passed: false,
                message: format!("Parse error: {:?}", e),
            },
        }
    }
    
    fn test_type_check_error(&self, input: &str, expected_error: &str) -> TestResult {
        match parse_program(input) {
            Ok((remaining, program)) => {
                if !remaining.trim().is_empty() {
                    return TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!("Unparsed input remains: {}", remaining),
                    };
                }
                
                let mut checker = TypeChecker::new();
                match checker.check_program(&program) {
                    Ok(()) => TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: "Expected type error but type checking succeeded".to_string(),
                    },
                    Err(e) => {
                        let error_msg = format!("{:?}", e);
                        if error_msg.contains(expected_error) {
                            TestResult {
                    duration: std::time::Duration::ZERO,
                                passed: true,
                                message: format!("Got expected type error: {}", error_msg),
                            }
                        } else {
                            TestResult {
                    duration: std::time::Duration::ZERO,
                                passed: false,
                                message: format!(
                                    "Got type error but didn't match expected. Got: {}, Expected to contain: {}",
                                    error_msg, expected_error
                                ),
                            }
                        }
                    }
                }
            }
            Err(e) => TestResult {
                    duration: std::time::Duration::ZERO,
                passed: false,
                message: format!("Parse error: {:?}", e),
            },
        }
    }
    
    fn test_codegen_contains(&self, input: &str, patterns: &[String]) -> TestResult {
        match parse_program(input) {
            Ok((remaining, program)) => {
                if !remaining.trim().is_empty() {
                    return TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!("Unparsed input remains: {}", remaining),
                    };
                }
                
                let mut checker = TypeChecker::new();
                if let Err(e) = checker.check_program(&program) {
                    return TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!("Type check error: {:?}", e),
                    };
                }
                
                let mut codegen = WasmCodeGen::new();
                match codegen.generate(&program) {
                    Ok(wat) => {
                        let mut missing = Vec::new();
                        for pattern in patterns {
                            if !wat.contains(pattern) {
                                missing.push(pattern.clone());
                            }
                        }
                        
                        if missing.is_empty() {
                            TestResult {
                    duration: std::time::Duration::ZERO,
                                passed: true,
                                message: "All expected patterns found in generated code".to_string(),
                            }
                        } else {
                            TestResult {
                    duration: std::time::Duration::ZERO,
                                passed: false,
                                message: format!("Missing patterns in generated code: {:?}", missing),
                            }
                        }
                    }
                    Err(e) => TestResult {
                    duration: std::time::Duration::ZERO,
                        passed: false,
                        message: format!("Code generation error: {:?}", e),
                    },
                }
            }
            Err(e) => TestResult {
                    duration: std::time::Duration::ZERO,
                passed: false,
                message: format!("Parse error: {:?}", e),
            },
        }
    }
    
    pub fn print_results(&self) {
        println!("\n=== Test Results ===\n");
        
        for (name, result) in &self.results {
            let status = if result.passed { "✓" } else { "✗" };
            println!("{} {} ({}ms)", status, name, result.duration.as_millis());
            if !result.passed {
                println!("  {}", result.message);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub failed_tests: Vec<String>,
}

impl TestSummary {
    pub fn print(&self) {
        println!("\n=== Test Summary ===");
        println!("Total:  {}", self.total);
        println!("Passed: {} ({}%)", self.passed, 
                 if self.total > 0 { self.passed * 100 / self.total } else { 0 });
        println!("Failed: {}", self.failed);
        
        if !self.failed_tests.is_empty() {
            println!("\nFailed tests:");
            for test in &self.failed_tests {
                println!("  - {}", test);
            }
        }
    }
}

/// Helper function to create test cases from a simple DSL
pub fn test_case(name: &str, input: &str) -> TestCase {
    TestCase {
        name: name.to_string(),
        description: String::new(),
        input: input.to_string(),
        expected: TestExpectation::TypeCheckSuccess,
        tags: vec![],
    }
}

#[cfg(test)]
mod framework_tests {
    use super::*;
    
    #[test]
    fn test_framework_basic() {
        let mut runner = TestRunner::new();
        
        runner.add_test(TestCase {
            name: "simple_function".to_string(),
            description: "Test simple function parsing".to_string(),
            input: "fun main: () -> Int = { Unit }".to_string(),
            expected: TestExpectation::ParseSuccess,
            tags: vec!["syntax".to_string()],
        });
        
        let summary = runner.run_all();
        assert_eq!(summary.total, 1);
    }
}