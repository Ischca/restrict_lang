use restrict_lang::ir::builder::build_checked_ir;
use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Module, Store};

fn compile_to_wasm(source: &str) -> Result<Vec<u8>, String> {
    let (remaining, program) =
        parse_program(source).map_err(|error| format!("parse error: {error:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("unparsed input remaining: {remaining:?}"));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|error| format!("type error: {error}"))?;
    let checked_ir = build_checked_ir(&program, &checker)
        .map_err(|error| format!("checked IR error: {error}"))?;
    let wat = WasmCodeGen::new()
        .generate_checked(&program, &checked_ir)
        .map_err(|error| format!("codegen error: {error}"))?;

    wat::parse_str(&wat).map_err(|error| format!("invalid generated WAT: {error}\n\n{wat}"))
}

fn instantiate(source: &str) -> Result<(Store<()>, Instance), Box<dyn std::error::Error>> {
    let wasm = compile_to_wasm(source)?;
    wasmparser::Validator::new().validate_all(&wasm)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..])?;
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_write",
        |_caller: Caller<'_, ()>, _fd: i32, _iovs: i32, _iovs_len: i32, _nwritten: i32| -> i32 {
            0
        },
    )?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "proc_exit",
        |_caller: Caller<'_, ()>, _code: i32| {},
    )?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    Ok((store, instance))
}

#[test]
fn qualified_enum_constructors_and_match_execute() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
pub enum CheckoutError {
    InvalidSku
    PaymentDeclined(String)
    RetryAfter(Int64)
}

fun classify: (error: CheckoutError) -> Int32 = {
    error match {
        CheckoutError::InvalidSku => { 10 }
        CheckoutError::PaymentDeclined(message) => { 20 }
        CheckoutError::RetryAfter(delay) => { 30 }
    }
}

export fun enum_case: (code: Int32) -> Int32 = {
    code == 0 then {
        () CheckoutError::InvalidSku |> classify
    } else {
        code == 1 then {
            "declined" |> CheckoutError::PaymentDeclined |> classify
        } else {
            60 as Int64 |> CheckoutError::RetryAfter |> classify
        }
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let enum_case = instance.get_typed_func::<i32, i32>(&store, "enum_case")?;

    assert_eq!(enum_case.call(&mut store, 0)?, 10);
    assert_eq!(enum_case.call(&mut store, 1)?, 20);
    assert_eq!(enum_case.call(&mut store, 2)?, 30);
    Ok(())
}

#[test]
fn custom_error_enum_flows_through_result() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
enum DecodeError {
    Empty
    Invalid(String)
}

fun decode: (code: Int32) -> Result<Int32, DecodeError> = {
    code == 0 then {
        Ok(42)
    } else {
        Err("invalid" |> DecodeError::Invalid)
    }
}

fun collapse: (result: Result<Int32, DecodeError>) -> Int32 = {
    result match {
        Ok(value) => { value }
        Err(error) => {
            error match {
                DecodeError::Empty => { -1 }
                DecodeError::Invalid(message) => { -2 }
            }
        }
    }
}

export fun result_case: (code: Int32) -> Int32 = {
    code |> decode |> collapse
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let result_case = instance.get_typed_func::<i32, i32>(&store, "result_case")?;

    assert_eq!(result_case.call(&mut store, 0)?, 42);
    assert_eq!(result_case.call(&mut store, 1)?, -2);
    Ok(())
}
