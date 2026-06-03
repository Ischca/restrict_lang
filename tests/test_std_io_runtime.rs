use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use wasmi::{Caller, Engine, Instance, Linker, Memory, Module, Store};

#[derive(Default)]
struct CapturedIo {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn compile_to_wasm(source: &str) -> Result<Vec<u8>, String> {
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {e:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {remaining:?}"));
    }

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {e}"))?;

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&ast)
        .map_err(|e| format!("Codegen error: {e}"))?;

    wat::parse_str(&wat).map_err(|e| format!("Invalid generated WAT: {e}\n\n{wat}"))
}

fn read_i32(memory: Memory, caller: &Caller<'_, CapturedIo>, offset: i32) -> Result<i32, i32> {
    let mut bytes = [0; 4];
    memory
        .read(caller, offset as usize, &mut bytes)
        .map_err(|_| 1)?;
    Ok(i32::from_le_bytes(bytes))
}

fn capture_fd_write(
    mut caller: Caller<'_, CapturedIo>,
    fd: i32,
    iovs: i32,
    iovs_len: i32,
    nwritten: i32,
) -> i32 {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        return 1;
    };

    let mut written = 0usize;
    let mut captured = Vec::new();
    for i in 0..iovs_len {
        let iov = iovs + (i * 8);
        let base = match read_i32(memory, &caller, iov) {
            Ok(base) => base,
            Err(errno) => return errno,
        };
        let len = match read_i32(memory, &caller, iov + 4) {
            Ok(len) => len,
            Err(errno) => return errno,
        };

        let mut bytes = vec![0; len as usize];
        if memory.read(&caller, base as usize, &mut bytes).is_err() {
            return 1;
        }
        written += bytes.len();
        captured.extend(bytes);
    }

    match fd {
        1 => caller.data_mut().stdout.extend(captured),
        2 => caller.data_mut().stderr.extend(captured),
        _ => return 8,
    }

    if nwritten != 0 {
        let bytes = (written as i32).to_le_bytes();
        if memory
            .write(&mut caller, nwritten as usize, &bytes)
            .is_err()
        {
            return 1;
        }
    }

    0
}

fn instantiate(source: &str) -> Result<(Store<CapturedIo>, Instance), Box<dyn std::error::Error>> {
    let wasm = compile_to_wasm(source)?;
    wasmparser::Validator::new().validate_all(&wasm)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..])?;
    let mut store = Store::new(&engine, CapturedIo::default());
    let mut linker = Linker::new(&engine);

    linker.func_wrap("wasi_snapshot_preview1", "fd_write", capture_fd_write)?;
    linker.func_wrap(
        "wasi_snapshot_preview1",
        "proc_exit",
        |_caller: Caller<'_, CapturedIo>, _code: i32| {},
    )?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    Ok((store, instance))
}

#[test]
fn std_io_functions_emit_expected_wasi_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun std_io_smoke: () -> () = {
    "Hello, " |> print;
    "Restrict" |> println;
    42 |> print_int;
    3.14 |> print_float;
    "warn: " |> eprint;
    "check" |> eprintln
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let std_io_smoke = instance.get_typed_func::<(), ()>(&store, "std_io_smoke")?;

    std_io_smoke.call(&mut store, ())?;

    assert_eq!(store.data().stdout, b"Hello, Restrict\n42\n3.14");
    assert_eq!(store.data().stderr, b"warn: check\n");
    Ok(())
}
