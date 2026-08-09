use quickcheck::{Arbitrary, Gen, QuickCheck};
use restrict_lang::{parse_program, type_check, Program, WasmCodeGen};

#[derive(Clone, Debug)]
struct ValidProgram(String);

impl Arbitrary for ValidProgram {
    fn arbitrary(g: &mut Gen) -> Self {
        let depth = *g.choose(&[0, 1, 2, 3]).unwrap();
        let function_count = *g.choose(&[1, 2, 3]).unwrap();
        let mut declarations = Vec::with_capacity(function_count);

        for index in 0..function_count {
            let name = if index == 0 {
                "main".to_string()
            } else {
                format!("f{index}")
            };

            declarations.push(format!(
                "fun {name}: () -> Int32 = {{\n    {}\n}}",
                generate_int_expr(g, depth)
            ));
        }

        ValidProgram(declarations.join("\n\n"))
    }
}

fn generate_int_expr(g: &mut Gen, depth: usize) -> String {
    if depth == 0 {
        return generate_int_literal(g);
    }

    match u8::arbitrary(g) % 4 {
        0 => generate_int_literal(g),
        _ => {
            let left = generate_int_expr(g, depth - 1);
            let right = generate_int_expr(g, depth - 1);
            let op = *g.choose(&["+", "-", "*"]).unwrap();
            format!("({left} {op} {right})")
        }
    }
}

fn generate_int_literal(g: &mut Gen) -> String {
    (u8::arbitrary(g) % 100).to_string()
}

fn parse_complete_program(source: &str) -> Result<Program, String> {
    let (remaining, program) =
        parse_program(source).map_err(|err| format!("parse failed: {err:?}\n{source}"))?;

    if !remaining.trim().is_empty() {
        return Err(format!(
            "parser left unconsumed input: {remaining:?}\n{source}"
        ));
    }

    Ok(program)
}

fn prop_generated_programs_parse(prog: ValidProgram) -> bool {
    parse_complete_program(&prog.0).is_ok()
}

fn prop_generated_programs_type_check(prog: ValidProgram) -> bool {
    parse_complete_program(&prog.0)
        .and_then(|program| type_check(&program).map_err(|err| format!("{err}")))
        .is_ok()
}

fn prop_generated_programs_generate_wat(prog: ValidProgram) -> bool {
    let program = match parse_complete_program(&prog.0) {
        Ok(program) => program,
        Err(_) => return false,
    };

    if type_check(&program).is_err() {
        return false;
    }

    let mut codegen = WasmCodeGen::new();
    match codegen.generate(&program) {
        Ok(wat) => wat.contains("(module") && wat.contains("(func"),
        Err(_) => false,
    }
}

#[test]
fn property_tests_generated_programs_parse() {
    QuickCheck::new()
        .tests(100)
        .quickcheck(prop_generated_programs_parse as fn(ValidProgram) -> bool);
}

#[test]
fn property_tests_generated_programs_type_check() {
    QuickCheck::new()
        .tests(100)
        .quickcheck(prop_generated_programs_type_check as fn(ValidProgram) -> bool);
}

#[test]
fn property_tests_generated_programs_generate_wat() {
    QuickCheck::new()
        .tests(100)
        .quickcheck(prop_generated_programs_generate_wat as fn(ValidProgram) -> bool);
}
