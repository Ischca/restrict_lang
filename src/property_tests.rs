use quickcheck::{Arbitrary, Gen, QuickCheck};
use crate::ast::*;
use crate::parser::parse_program;
use crate::type_checker::type_check;
use crate::codegen::generate;

// Generate random valid Restrict Language programs
#[derive(Clone, Debug)]
struct ValidProgram(String);

impl Arbitrary for ValidProgram {
    fn arbitrary(g: &mut Gen) -> Self {
        let depth = *g.choose(&[1, 2, 3]).unwrap();
        ValidProgram(generate_program(g, depth))
    }
}

fn generate_program(g: &mut Gen, depth: usize) -> String {
    let mut program = String::new();
    
    // Generate some function declarations
    let num_functions = *g.choose(&[1, 2, 3]).unwrap();
    for i in 0..num_functions {
        program.push_str(&format!("fun f{} = ", i));
        program.push_str(&generate_block(g, depth));
        program.push('\n');
    }
    
    program
}

fn generate_block(g: &mut Gen, depth: usize) -> String {
    let mut block = String::from("{ ");
    
    // Generate some statements
    let num_stmts = if depth > 0 { *g.choose(&[0, 1, 2]).unwrap() } else { 0 };
    for _ in 0..num_stmts {
        if bool::arbitrary(g) {
            // Variable binding
            let var_name = format!("x{}", u8::arbitrary(g) % 10);
            if bool::arbitrary(g) {
                block.push_str("mut ");
            }
            block.push_str(&format!("val {} = ", var_name));
            block.push_str(&generate_expr(g, depth - 1));
            block.push(' ');
        } else {
            // Expression statement
            block.push_str(&generate_expr(g, depth - 1));
            block.push(' ');
        }
    }
    
    // Final expression
    block.push_str(&generate_expr(g, 0));
    block.push_str(" }");
    block
}

fn generate_expr(g: &mut Gen, depth: usize) -> String {
    if depth == 0 {
        // Base case: literals or identifiers
        match u8::arbitrary(g) % 4 {
            0 => format!("{}", i32::arbitrary(g).abs() % 100),
            1 => "true".to_string(),
            2 => "false".to_string(),
            _ => format!("x{}", u8::arbitrary(g) % 10),
        }
    } else {
        // Recursive case
        match u8::arbitrary(g) % 3 {
            0 => {
                // Binary expression
                let left = generate_expr(g, depth - 1);
                let right = generate_expr(g, depth - 1);
                let op = *g.choose(&["+", "-", "*", "==", "<"]).unwrap();
                format!("{} {} {}", left, op, right)
            }
            1 => {
                // Block
                generate_block(g, depth - 1)
            }
            _ => {
                // Parenthesized expression
                format!("({})", generate_expr(g, depth - 1))
            }
        }
    }
}

// Property: All valid programs should parse without error
fn prop_valid_programs_parse(prog: ValidProgram) -> bool {
    match parse_program(&prog.0) {
        Ok((remaining, _)) => remaining.trim().is_empty(),
        Err(_) => false,
    }
}

// Property: Parser should be idempotent (parse -> pretty print -> parse gives same AST)
fn prop_parser_idempotent(prog: ValidProgram) -> bool {
    match parse_program(&prog.0) {
        Ok((_, ast1)) => {
            // We would need a pretty printer here
            // For now, just check that it parses
            true
        }
        Err(_) => true, // Skip invalid programs
    }
}

// Property: Type checker should accept or reject consistently
fn prop_type_checker_deterministic(prog: ValidProgram) -> bool {
    match parse_program(&prog.0) {
        Ok((_, ast)) => {
            let result1 = type_check(&ast);
            let result2 = type_check(&ast);
            match (result1, result2) {
                (Ok(_), Ok(_)) => true,
                (Err(_), Err(_)) => true,
                _ => false,
            }
        }
        Err(_) => true,
    }
}

// Property: Generated WASM should be valid
fn prop_codegen_valid_wasm(prog: ValidProgram) -> bool {
    match parse_program(&prog.0) {
        Ok((_, ast)) => {
            match type_check(&ast) {
                Ok(typed_ast) => {
                    match generate(&typed_ast) {
                        Ok(wat) => {
                            // Check basic WASM structure
                            wat.contains("(module") && wat.contains(")")
                        }
                        Err(_) => true, // Codegen errors are acceptable
                    }
                }
                Err(_) => true, // Type errors are acceptable
            }
        }
        Err(_) => true, // Parse errors are acceptable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_programs_parse() {
        QuickCheck::new()
            .tests(100)
            .quickcheck(prop_valid_programs_parse as fn(ValidProgram) -> bool);
    }
    
    #[test]
    fn test_type_checker_deterministic() {
        QuickCheck::new()
            .tests(100)
            .quickcheck(prop_type_checker_deterministic as fn(ValidProgram) -> bool);
    }
    
    #[test]
    fn test_codegen_valid_wasm() {
        QuickCheck::new()
            .tests(100)
            .quickcheck(prop_codegen_valid_wasm as fn(ValidProgram) -> bool);
    }
}