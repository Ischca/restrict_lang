use restrict_lang::ir::builder::build_checked_ir;
use restrict_lang::{
    check_release_surface, parse_program, HostAbiProfile, TypeChecker, WasmCodeGen,
    WasmOptimizationLevel, WasmTargetProfile,
};
use wasmi::{Caller, Engine, Instance, Linker, Memory, Module, Store};
use wasmparser::{Parser, Payload, Validator};

#[derive(Default)]
struct WasiTestEnv {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdin: Vec<u8>,
    stdin_cursor: usize,
    args: Vec<String>,
    env_vars: Vec<(String, String)>,
    exit_code: Option<i32>,
}

fn compile_wat_with_options(
    source: &str,
    target: WasmTargetProfile,
    arena_bytes: u32,
    optimization: WasmOptimizationLevel,
) -> anyhow::Result<(String, Option<restrict_lang::ReleaseOptimizationReport>)> {
    let (remaining, program) =
        parse_program(source).map_err(|error| anyhow::anyhow!("Parse error: {error:?}"))?;
    anyhow::ensure!(
        remaining.trim().is_empty(),
        "Unparsed input remaining: {remaining:?}"
    );

    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&program)?;
    check_release_surface(&program, &type_checker, HostAbiProfile::V001Scalar)?;
    let checked_ir = build_checked_ir(&program, &type_checker)?;
    let mut codegen = WasmCodeGen::with_host_abi_profile(HostAbiProfile::V001Scalar)
        .with_target_profile(target)
        .with_optimization_level(optimization)
        .with_arena_size_bytes(arena_bytes)?;
    let wat = codegen.generate_checked(&program, &checked_ir)?;
    Ok((wat, codegen.optimization_report()))
}

fn compile_wasm(
    source: &str,
    target: WasmTargetProfile,
    optimization: WasmOptimizationLevel,
) -> anyhow::Result<Vec<u8>> {
    let (wat, _) = compile_wat_with_options(source, target, 16_384, optimization)?;
    let wasm = wat::parse_str(&wat)?;
    Validator::new().validate_all(&wasm)?;
    Ok(wasm)
}

fn import_names(wasm: &[u8]) -> anyhow::Result<Vec<(String, String)>> {
    let mut imports = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::ImportSection(section) = payload? {
            for import in section.into_imports() {
                let import = import?;
                imports.push((import.module.to_string(), import.name.to_string()));
            }
        }
    }
    Ok(imports)
}

fn get_memory(caller: &mut Caller<'_, WasiTestEnv>) -> Option<Memory> {
    caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
}

fn write_u32(memory: Memory, caller: &mut Caller<'_, WasiTestEnv>, offset: i32, val: u32) -> i32 {
    if memory
        .write(caller, offset as usize, &val.to_le_bytes())
        .is_err()
    {
        1
    } else {
        0
    }
}

fn read_u32(memory: Memory, caller: &Caller<'_, WasiTestEnv>, offset: i32) -> Result<u32, i32> {
    let mut buf = [0u8; 4];
    memory
        .read(caller, offset as usize, &mut buf)
        .map_err(|_| 1)?;
    Ok(u32::from_le_bytes(buf))
}

fn call_start(instance: &Instance, store: &mut Store<WasiTestEnv>) -> Result<(), wasmi::Error> {
    let start = instance.get_typed_func::<(), ()>(&*store, "_start")?;
    start.call(store, ())
}

fn instantiate_with_env(
    wasm: &[u8],
    env: WasiTestEnv,
) -> anyhow::Result<(Store<WasiTestEnv>, Instance)> {
    let engine = Engine::default();
    let module = Module::new(&engine, wasm)?;
    let mut store = Store::new(&engine, env);
    let mut linker = Linker::new(&engine);

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_write",
        |mut caller: Caller<'_, WasiTestEnv>,
         fd: i32,
         iovs: i32,
         iovs_len: i32,
         nwritten: i32|
         -> i32 {
            let Some(memory) = get_memory(&mut caller) else {
                return 1;
            };
            let mut total = 0u32;
            let mut captured = Vec::new();
            for i in 0..iovs_len {
                let iov = iovs + i * 8;
                let Ok(base) = read_u32(memory, &caller, iov) else {
                    return 1;
                };
                let Ok(len) = read_u32(memory, &caller, iov + 4) else {
                    return 1;
                };
                let mut bytes = vec![0u8; len as usize];
                if memory.read(&caller, base as usize, &mut bytes).is_err() {
                    return 1;
                }
                total += len;
                captured.extend(bytes);
            }
            match fd {
                1 => caller.data_mut().stdout.extend(captured),
                2 => caller.data_mut().stderr.extend(captured),
                _ => return 8,
            }
            if nwritten != 0 && write_u32(memory, &mut caller, nwritten, total) != 0 {
                return 1;
            }
            0
        },
    )?;

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "proc_exit",
        |mut caller: Caller<'_, WasiTestEnv>, code: i32| -> Result<(), wasmi::Error> {
            caller.data_mut().exit_code = Some(code);
            Err(wasmi::Error::i32_exit(code))
        },
    )?;

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "args_sizes_get",
        |mut caller: Caller<'_, WasiTestEnv>, argc_ptr: i32, buf_size_ptr: i32| -> i32 {
            let Some(memory) = get_memory(&mut caller) else {
                return 1;
            };
            let argc = caller.data().args.len() as u32;
            let buf_size: u32 = caller
                .data()
                .args
                .iter()
                .map(|arg| (arg.len() + 1) as u32)
                .sum();
            if write_u32(memory, &mut caller, argc_ptr, argc) != 0 {
                return 1;
            }
            write_u32(memory, &mut caller, buf_size_ptr, buf_size)
        },
    )?;

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "args_get",
        |mut caller: Caller<'_, WasiTestEnv>, argv_ptr: i32, buf_ptr: i32| -> i32 {
            let Some(memory) = get_memory(&mut caller) else {
                return 1;
            };
            let args = caller.data().args.clone();
            let mut cursor = buf_ptr as u32;
            for (idx, arg) in args.iter().enumerate() {
                if write_u32(memory, &mut caller, argv_ptr + (idx as i32) * 4, cursor) != 0 {
                    return 1;
                }
                let mut bytes = arg.as_bytes().to_vec();
                bytes.push(0);
                if memory.write(&mut caller, cursor as usize, &bytes).is_err() {
                    return 1;
                }
                cursor += bytes.len() as u32;
            }
            0
        },
    )?;

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "environ_sizes_get",
        |mut caller: Caller<'_, WasiTestEnv>, count_ptr: i32, buf_size_ptr: i32| -> i32 {
            let Some(memory) = get_memory(&mut caller) else {
                return 1;
            };
            let count = caller.data().env_vars.len() as u32;
            let buf_size: u32 = caller
                .data()
                .env_vars
                .iter()
                .map(|(k, v)| (k.len() + 1 + v.len() + 1) as u32)
                .sum();
            if write_u32(memory, &mut caller, count_ptr, count) != 0 {
                return 1;
            }
            write_u32(memory, &mut caller, buf_size_ptr, buf_size)
        },
    )?;

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "environ_get",
        |mut caller: Caller<'_, WasiTestEnv>, envp_ptr: i32, buf_ptr: i32| -> i32 {
            let Some(memory) = get_memory(&mut caller) else {
                return 1;
            };
            let vars = caller.data().env_vars.clone();
            let mut cursor = buf_ptr as u32;
            for (idx, (k, v)) in vars.iter().enumerate() {
                if write_u32(memory, &mut caller, envp_ptr + (idx as i32) * 4, cursor) != 0 {
                    return 1;
                }
                let mut bytes = Vec::with_capacity(k.len() + v.len() + 2);
                bytes.extend_from_slice(k.as_bytes());
                bytes.push(b'=');
                bytes.extend_from_slice(v.as_bytes());
                bytes.push(0);
                if memory.write(&mut caller, cursor as usize, &bytes).is_err() {
                    return 1;
                }
                cursor += bytes.len() as u32;
            }
            0
        },
    )?;

    linker.func_wrap(
        "wasi_snapshot_preview1",
        "fd_read",
        |mut caller: Caller<'_, WasiTestEnv>,
         fd: i32,
         iovs: i32,
         iovs_len: i32,
         nread_ptr: i32|
         -> i32 {
            if fd != 0 {
                return 8;
            }
            let Some(memory) = get_memory(&mut caller) else {
                return 1;
            };
            let mut total_read = 0u32;
            for i in 0..iovs_len {
                let iov = iovs + i * 8;
                let Ok(base) = read_u32(memory, &caller, iov) else {
                    return 1;
                };
                let Ok(len) = read_u32(memory, &caller, iov + 4) else {
                    return 1;
                };
                let avail = caller
                    .data()
                    .stdin
                    .len()
                    .saturating_sub(caller.data().stdin_cursor);
                let to_copy = avail.min(len as usize).min(64);
                if to_copy == 0 {
                    break;
                }
                let start = caller.data().stdin_cursor;
                let chunk = caller.data().stdin[start..start + to_copy].to_vec();
                if memory.write(&mut caller, base as usize, &chunk).is_err() {
                    return 1;
                }
                caller.data_mut().stdin_cursor += to_copy;
                total_read += to_copy as u32;
            }
            if nread_ptr != 0 && write_u32(memory, &mut caller, nread_ptr, total_read) != 0 {
                return 1;
            }
            0
        },
    )?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    Ok((store, instance))
}

#[test]
fn string_inspection_slicing_search_and_utf8_runtime() -> anyhow::Result<()> {
    let source = r#"
export fun test_lengths: () -> Int32 = {
    val ascii = "hello";
    val jp = "あいう";
    val empty = "";
    val a = ascii |> string_length;
    val b = jp |> string_byte_length;
    val c = empty |> string_length;
    a * 100 + b * 10 + c
}

export fun test_byte_at: () -> Int32 = {
    mut val s = "ABあ";
    val b0 = ((s, 0) string_byte_at, -1) option_unwrap_or;
    val b2 = ((s, 2) string_byte_at, -1) option_unwrap_or;
    val neg = ((s, -1) string_byte_at, -2) option_unwrap_or;
    val oob = ((s, 5) string_byte_at, -3) option_unwrap_or;
    b0 + b2 + neg + oob
}

export fun test_slice_valid_jp: () -> Boolean = {
    val s = "あいう";
    val sliced = ((s, 0, 3) string_slice, "err") option_unwrap_or;
    sliced == "あ"
}

export fun test_slice_tail_jp: () -> Boolean = {
    val s = "あいう";
    val sliced = ((s, 3, 9) string_slice, "err") option_unwrap_or;
    sliced == "いう"
}

export fun test_slice_invalid_boundaries: () -> Int32 = {
    mut val s = "あいう";
    val mid_start = (s, 1, 3) string_slice |> option_is_none;
    val mid_end = (s, 0, 2) string_slice |> option_is_none;
    val neg = (s, -1, 3) string_slice |> option_is_none;
    val inverted = (s, 6, 3) string_slice |> option_is_none;
    val past_end = (s, 0, 10) string_slice |> option_is_none;
    val empty_end = ((s, 9, 9) string_slice, "err") option_unwrap_or;
    (mid_start then { 1 } else { 0 })
        + (mid_end then { 2 } else { 0 })
        + (neg then { 4 } else { 0 })
        + (inverted then { 8 } else { 0 })
        + (past_end then { 16 } else { 0 })
        + (empty_end == "" then { 32 } else { 0 })
}

export fun test_search_and_predicates: () -> Int32 = {
    mut val text = "東京-大阪-京都";
    val idx1 = ((text, "大阪") string_index_of, -1) option_unwrap_or;
    val idx_empty = ((text, "") string_index_of, -1) option_unwrap_or;
    val idx_missing = ((text, "札幌") string_index_of, -9) option_unwrap_or;
    val has = (text, "大阪") string_contains;
    val not_has = (text, "福岡") string_contains;
    val sw = (text, "東京") string_starts_with;
    val ew = (text, "京都") string_ends_with;
    val bad_sw = ("a", "ab") string_starts_with;
    val bad_ew = ("a", "ab") string_ends_with;
    idx1 * 100
        + idx_empty * 10
        + (idx_missing == -9 then { 1 } else { 0 })
        + (has then { 2 } else { 0 })
        + (not_has then { 0 } else { 4 })
        + (sw then { 8 } else { 0 })
        + (ew then { 16 } else { 0 })
        + (bad_sw then { 0 } else { 32 })
        + (bad_ew then { 0 } else { 64 })
}
"#;

    for opt in [WasmOptimizationLevel::None, WasmOptimizationLevel::Release] {
        let wasm = compile_wasm(source, WasmTargetProfile::WasmCore, opt)?;
        assert!(import_names(&wasm)?.is_empty());
        let (mut store, instance) = instantiate_with_env(&wasm, WasiTestEnv::default())?;

        let lengths = instance
            .get_typed_func::<(), i32>(&store, "test_lengths")?
            .call(&mut store, ())?;
        assert_eq!(lengths, 590);

        let byte_at = instance
            .get_typed_func::<(), i32>(&store, "test_byte_at")?
            .call(&mut store, ())?;
        assert_eq!(byte_at, 65 + 0xE3 - 2 - 3);

        let slice1 = instance
            .get_typed_func::<(), i32>(&store, "test_slice_valid_jp")?
            .call(&mut store, ())?;
        assert_eq!(slice1, 1);

        let slice2 = instance
            .get_typed_func::<(), i32>(&store, "test_slice_tail_jp")?
            .call(&mut store, ())?;
        assert_eq!(slice2, 1);

        let invalid = instance
            .get_typed_func::<(), i32>(&store, "test_slice_invalid_boundaries")?
            .call(&mut store, ())?;
        assert_eq!(invalid, 1 + 2 + 4 + 8 + 16 + 32);

        let search = instance
            .get_typed_func::<(), i32>(&store, "test_search_and_predicates")?
            .call(&mut store, ())?;
        // "東京-" is 6 + 1 = 7 bytes
        assert_eq!(search, 7 * 100 + 1 + 2 + 4 + 8 + 16 + 32 + 64);
    }

    Ok(())
}

#[test]
fn string_trim_split_join_and_number_parsers_runtime() -> anyhow::Result<()> {
    let source = r#"
export fun test_trim: () -> Boolean = {
    val trimmed = " \t\n 東京・大阪 \r\n " |> string_trim;
    trimmed == "東京・大阪"
}

export fun test_trim_empty: () -> Boolean = {
    val trimmed = "  \t\r\n  " |> string_trim;
    trimmed == ""
}

export fun test_split_join_csv: () -> Boolean = {
    val parts = ("東京,,大阪,京都,", ",") string_split;
    val joined = (parts, "|") string_join;
    joined == "東京||大阪|京都|"
}

export fun test_split_empty_sep_utf8: () -> Boolean = {
    val chars = ("あaい🚀", "") string_split;
    val empty_chars = ("", "") string_split;
    val count = ("あaい🚀", "") string_split |> list_length;
    val empty_count = empty_chars list_length;
    val joined = (chars, "-") string_join;
    val expected = "あ-a-い-🚀:4:0";
    (joined + ":" + (count |> display) + ":" + (empty_count |> display)) == expected
}

export fun test_join_empty: () -> Boolean = {
    val empty: List<String> = [];
    ((empty, ",") string_join) == ""
}

export fun test_parse_int32: () -> Int32 = {
    val a = ("  -42\t" string_parse_int, 0) option_unwrap_or;
    val b = ("+2147483647" string_parse_int, 0) option_unwrap_or;
    val c = ("-2147483648" string_parse_int, 0) option_unwrap_or;
    val over = ("2147483648" string_parse_int) option_is_none;
    val under = ("-2147483649" string_parse_int) option_is_none;
    val bad = ("12x" string_parse_int) option_is_none;
    val empty = ("   " string_parse_int) option_is_none;
    val plus = ("+" string_parse_int) option_is_none;
    val ok_min = c == (-2147483647 - 1);
    val ok_max = b == 2147483647;
    a
        + (ok_min then { 100 } else { 0 })
        + (ok_max then { 200 } else { 0 })
        + (over then { 1 } else { 0 })
        + (under then { 2 } else { 0 })
        + (bad then { 4 } else { 0 })
        + (empty then { 8 } else { 0 })
        + (plus then { 16 } else { 0 })
}

export fun test_parse_int64: () -> Int64 = {
    val big = ("  9223372036854775807 " string_parse_int64, 0) option_unwrap_or;
    val min = ("-9223372036854775808" string_parse_int64, 0) option_unwrap_or;
    val over = ("9223372036854775808" string_parse_int64) option_is_none;
    val under = ("-9223372036854775809" string_parse_int64) option_is_none;
    val invalid = ("abc" string_parse_int64) option_is_none;
    val check = (big == 9223372036854775807)
        && (min == (-9223372036854775807 - 1))
        && over
        && under
        && invalid;
    val some_val = (" 5000000000 " string_parse_int64, 42) option_unwrap_or;
    check then { some_val } else { -1 }
}

export fun test_parse_float: () -> Float64 = {
    val a = (" 3.14 " string_parse_float, 0.0) option_unwrap_or;
    val b = ("-0.005" string_parse_float, 0.0) option_unwrap_or;
    val c = ("1.5e2" string_parse_float, 0.0) option_unwrap_or;
    val d = ("2.5E-2" string_parse_float, 0.0) option_unwrap_or;
    val over = ("1e309" string_parse_float) option_is_none;
    val dot = ("." string_parse_float) option_is_none;
    val bad_exp = ("1e" string_parse_float) option_is_none;
    val bad_char = ("1.2x" string_parse_float) option_is_none;
    val valid = over && dot && bad_exp && bad_char;
    valid then { a + b + c + d } else { -1.0 }
}
"#;

    let wasm = compile_wasm(
        source,
        WasmTargetProfile::WasmCore,
        WasmOptimizationLevel::Release,
    )?;
    let (mut store, instance) = instantiate_with_env(&wasm, WasiTestEnv::default())?;

    let trimmed = instance
        .get_typed_func::<(), i32>(&store, "test_trim")?
        .call(&mut store, ())?;
    assert_eq!(trimmed, 1);

    let trim_empty = instance
        .get_typed_func::<(), i32>(&store, "test_trim_empty")?
        .call(&mut store, ())?;
    assert_eq!(trim_empty, 1);

    let csv = instance
        .get_typed_func::<(), i32>(&store, "test_split_join_csv")?
        .call(&mut store, ())?;
    assert_eq!(csv, 1);

    let utf8_split = instance
        .get_typed_func::<(), i32>(&store, "test_split_empty_sep_utf8")?
        .call(&mut store, ())?;
    assert_eq!(utf8_split, 1);

    let join_empty = instance
        .get_typed_func::<(), i32>(&store, "test_join_empty")?
        .call(&mut store, ())?;
    assert_eq!(join_empty, 1);

    let pi = instance
        .get_typed_func::<(), i32>(&store, "test_parse_int32")?
        .call(&mut store, ())?;
    assert_eq!(pi, -42 + 100 + 200 + 1 + 2 + 4 + 8 + 16);

    let pi64 = instance
        .get_typed_func::<(), i64>(&store, "test_parse_int64")?
        .call(&mut store, ())?;
    assert_eq!(pi64, 5_000_000_000);

    let pf = instance
        .get_typed_func::<(), f64>(&store, "test_parse_float")?
        .call(&mut store, ())?;
    #[allow(clippy::approx_constant)]
    let expected = 3.14 - 0.005 + 150.0 + 0.025;
    assert!(
        (pf - expected).abs() < 1e-12,
        "got {pf}, expected {expected}"
    );
    Ok(())
}

#[test]
fn math_char_array_option_i64_and_range_builtins_runtime() -> anyhow::Result<()> {
    let source = r#"
export fun test_float_math: () -> Float64 = {
    (9.0 |> sqrt)
        + (2.9 |> floor)
        + (2.1 |> ceil)
        + (2.5 |> round)
        + (-2.9 |> trunc)
}

export fun test_char_builtins: () -> Int32 = {
    val a = 'A' |> char_code;
    val jp = 'あ' |> char_code;
    val decoded = (0x1F680 |> char_from_code, 'x') option_unwrap_or;
    val surrogate = 0xD800 |> char_from_code |> option_is_none;
    val too_high = 0x110000 |> char_from_code |> option_is_none;
    val neg = -1 |> char_from_code |> option_is_none;
    val ok = surrogate && too_high && neg;
    ok then { a + jp + (decoded |> char_code) } else { -1 }
}

export fun test_array_length: () -> Int32 = {
    val ints: Array<Int32, 4> = [10, 20, 30, 40];
    val floats: Array<Float64, 3> = [1.0, 2.0, 3.0];
    val i64s: Array<Int64, 2> = [1, 2];
    (ints |> array_length) * 100 + (floats |> array_length) * 10 + (i64s |> array_length)
}

export fun test_option_i64_unwrap: () -> Int64 = {
    val some_v: Option<Int64> = 7000000000 Option::Some;
    val none_v: Option<Int64> = () Option::None;
    (some_v, 1) option_unwrap_or + (none_v, 42) option_unwrap_or
}

export fun test_range_builtins: () -> Int32 = {
    val r1 = [-2..2];
    val s = r1 |> range_start;
    val e = [-2..2] |> range_end;
    val xs = [-2..2] |> range_to_list;
    val empty1 = [5..5] |> range_to_list |> list_length;
    val empty2 = [5..2] |> range_to_list |> list_length;
    val sum = (xs, 0, |acc: Int32, x: Int32| acc + x) fold;
    val count = [-2..2] |> range_to_list |> list_length;
    s * 1000 + e * 100 + count * 10 + sum + empty1 + empty2
}
"#;

    let wasm = compile_wasm(
        source,
        WasmTargetProfile::WasmCore,
        WasmOptimizationLevel::Release,
    )?;
    assert!(import_names(&wasm)?.is_empty());
    let (mut store, instance) = instantiate_with_env(&wasm, WasiTestEnv::default())?;

    let math = instance
        .get_typed_func::<(), f64>(&store, "test_float_math")?
        .call(&mut store, ())?;
    // 3.0 + 2.0 + 3.0 + 2.0 + (-2.0) = 8.0
    assert_eq!(math, 8.0);

    let chars = instance
        .get_typed_func::<(), i32>(&store, "test_char_builtins")?
        .call(&mut store, ())?;
    assert_eq!(chars, 65 + 0x3042 + 0x1F680);

    let arr_len = instance
        .get_typed_func::<(), i32>(&store, "test_array_length")?
        .call(&mut store, ())?;
    assert_eq!(arr_len, 432);

    let opt_i64 = instance
        .get_typed_func::<(), i64>(&store, "test_option_i64_unwrap")?
        .call(&mut store, ())?;
    assert_eq!(opt_i64, 7_000_000_042);

    let range_res = instance
        .get_typed_func::<(), i32>(&store, "test_range_builtins")?
        .call(&mut store, ())?;
    // s=-2, e=2, count=4, sum=-2+-1+0+1=-2 -> -2000 + 200 + 40 - 2 = -1762
    assert_eq!(range_res, -1762);
    Ok(())
}

#[test]
fn panic_assert_stderr_and_exit_builtin_in_wasip1() -> anyhow::Result<()> {
    let panic_src = r#"
fun main: () -> () = {
    "fatal: bad input" |> panic
}
"#;
    let panic_wasm = compile_wasm(
        panic_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::None,
    )?;
    let (mut panic_store, panic_instance) =
        instantiate_with_env(&panic_wasm, WasiTestEnv::default())?;
    let panic_err = call_start(&panic_instance, &mut panic_store);
    assert!(panic_err.is_err(), "panic should trap");
    assert_eq!(panic_store.data().stderr, b"fatal: bad input\n");

    let assert_src = r#"
export fun ok_assert: () -> Int32 = {
    (true, "should not print") assert;
    7
}
export fun fail_assert: () -> () = {
    (false, "assert failed: expected > 0") assert
}
"#;
    let assert_wasm = compile_wasm(
        assert_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::Release,
    )?;
    let (mut store, instance) = instantiate_with_env(&assert_wasm, WasiTestEnv::default())?;
    assert_eq!(
        instance
            .get_typed_func::<(), i32>(&store, "ok_assert")?
            .call(&mut store, ())?,
        7
    );
    assert!(store.data().stderr.is_empty());
    let fail = instance
        .get_typed_func::<(), ()>(&store, "fail_assert")?
        .call(&mut store, ());
    assert!(fail.is_err());
    assert_eq!(store.data().stderr, b"assert failed: expected > 0\n");

    let exit_src = r#"
fun main: () -> () = {
    42 |> exit
}
"#;
    let exit_wasm = compile_wasm(
        exit_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::Release,
    )?;
    let (mut exit_store, exit_instance) = instantiate_with_env(&exit_wasm, WasiTestEnv::default())?;
    let err =
        call_start(&exit_instance, &mut exit_store).expect_err("exit(42) should call proc_exit");
    assert_eq!(err.i32_exit_status(), Some(42));
    assert_eq!(exit_store.data().exit_code, Some(42));

    // Ensure main() -> Int32 still returns normally from _start without calling proc_exit
    let normal_main_src = r#"
fun main: () -> Int32 = {
    42
}
"#;
    let normal_wasm = compile_wasm(
        normal_main_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::None,
    )?;
    let (mut store, instance) = instantiate_with_env(&normal_wasm, WasiTestEnv::default())?;
    call_start(&instance, &mut store)?;
    assert_eq!(store.data().exit_code, None);

    Ok(())
}

#[test]
fn on_demand_wasi_cli_args_env_get_and_stdin_read_all() -> anyhow::Result<()> {
    // 1. Program without new WASI builtins imports only fd_write and proc_exit
    let plain_src = r#"
fun main: () -> () = {
    "hi" |> println
}
"#;
    let plain_wasm = compile_wasm(
        plain_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::None,
    )?;
    let plain_imports = import_names(&plain_wasm)?;
    assert_eq!(
        plain_imports,
        vec![
            ("wasi_snapshot_preview1".to_string(), "fd_write".to_string()),
            (
                "wasi_snapshot_preview1".to_string(),
                "proc_exit".to_string()
            ),
        ]
    );

    // 2. cli_args only imports args_sizes_get + args_get (on demand)
    let args_src = r#"
fun main: () -> () = {
    val args = () cli_args;
    (args, "|") string_join |> print
}
"#;
    let args_wasm = compile_wasm(
        args_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::Release,
    )?;
    let args_imports = import_names(&args_wasm)?;
    assert!(args_imports.contains(&(
        "wasi_snapshot_preview1".to_string(),
        "args_sizes_get".to_string()
    )));
    assert!(args_imports.contains(&("wasi_snapshot_preview1".to_string(), "args_get".to_string())));
    assert!(!args_imports
        .iter()
        .any(|(_, n)| n == "environ_get" || n == "fd_read"));

    let (mut store, instance) = instantiate_with_env(
        &args_wasm,
        WasiTestEnv {
            args: vec!["prog".into(), "--lang".into(), "日本語".into()],
            ..Default::default()
        },
    )?;
    call_start(&instance, &mut store)?;
    assert_eq!(
        String::from_utf8(store.data().stdout.clone())?,
        "prog|--lang|日本語"
    );

    // 3. env_get only imports environ_sizes_get + environ_get
    let env_src = r#"
fun main: () -> () = {
    val a = ("APP" env_get, "missing") option_unwrap_or;
    val b = ("APP_ENV" env_get, "missing") option_unwrap_or;
    val c = ("NONE_SUCH" env_get, "missing") option_unwrap_or;
    (a + "|" + b + "|" + c) |> print
}
"#;
    let env_wasm = compile_wasm(
        env_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::Release,
    )?;
    let env_imports = import_names(&env_wasm)?;
    assert!(env_imports.contains(&(
        "wasi_snapshot_preview1".to_string(),
        "environ_sizes_get".to_string()
    )));
    assert!(env_imports.contains(&(
        "wasi_snapshot_preview1".to_string(),
        "environ_get".to_string()
    )));
    assert!(!env_imports
        .iter()
        .any(|(_, n)| n == "args_get" || n == "fd_read"));

    let (mut store, instance) = instantiate_with_env(
        &env_wasm,
        WasiTestEnv {
            env_vars: vec![("APP_ENV".into(), "生産".into())],
            ..Default::default()
        },
    )?;
    call_start(&instance, &mut store)?;
    assert_eq!(
        String::from_utf8(store.data().stdout.clone())?,
        "missing|生産|missing"
    );

    // 4. stdin_read_all only imports fd_read and handles multi-chunk reads (> 128 bytes)
    let stdin_src = r#"
fun main: () -> () = {
    () stdin_read_all |> print
}
"#;
    let stdin_wasm = compile_wasm(
        stdin_src,
        WasmTargetProfile::WasiP1,
        WasmOptimizationLevel::Release,
    )?;
    let stdin_imports = import_names(&stdin_wasm)?;
    assert!(stdin_imports.contains(&("wasi_snapshot_preview1".to_string(), "fd_read".to_string())));
    assert!(!stdin_imports
        .iter()
        .any(|(_, n)| n == "args_get" || n == "environ_get"));

    let long_input = "line-東京-".repeat(40); // > 400 bytes, triggers buffer growth (128 -> 256 -> 512)
    let (mut store, instance) = instantiate_with_env(
        &stdin_wasm,
        WasiTestEnv {
            stdin: long_input.as_bytes().to_vec(),
            ..Default::default()
        },
    )?;
    call_start(&instance, &mut store)?;
    assert_eq!(String::from_utf8(store.data().stdout.clone())?, long_input);

    // Empty stdin
    let (mut empty_store, empty_instance) =
        instantiate_with_env(&stdin_wasm, WasiTestEnv::default())?;
    call_start(&empty_instance, &mut empty_store)?;
    assert_eq!(String::from_utf8(empty_store.data().stdout.clone())?, "");

    Ok(())
}

#[test]
fn wasm_core_rejects_wasi_builtins_and_dce_strips_unused_imports() -> anyhow::Result<()> {
    for (name, snippet) in [
        (
            "cli_args",
            "fun main: () -> () = { val _a = () cli_args; () }",
        ),
        (
            "env_get",
            "fun main: () -> () = { val _e = \"K\" env_get; () }",
        ),
        (
            "stdin_read_all",
            "fun main: () -> () = { val _s = () stdin_read_all; () }",
        ),
        ("exit", "fun main: () -> () = { 1 |> exit }"),
        ("eprintln", "fun main: () -> () = { \"err\" |> eprintln }"),
    ] {
        let err = compile_wat_with_options(
            snippet,
            WasmTargetProfile::WasmCore,
            4096,
            WasmOptimizationLevel::None,
        )
        .expect_err(&format!("wasm-core must reject {name}"));
        let msg = err.to_string();
        assert!(
            msg.contains("wasip1") && msg.contains("host I/O"),
            "unexpected error for {name}: {msg}"
        );
    }

    // Dead helper using cli_args/env_get/stdin_read_all should have its WASI imports eliminated under --release
    let dce_src = r#"
fun unused_cli: () -> Int32 = {
    val _a = () cli_args;
    val _e = "X" env_get;
    val _s = () stdin_read_all;
    0
}
export fun pure_add: (a: Int32, b: Int32) -> Int32 = {
    a + b
}
"#;
    let (wat, report) = compile_wat_with_options(
        dce_src,
        WasmTargetProfile::WasiP1,
        4096,
        WasmOptimizationLevel::Release,
    )?;
    let report = report.expect("release report should be present");
    assert!(report.removed_functions > 0);
    assert!(report.removed_function_imports > 0);
    let wasm = wat::parse_str(&wat)?;
    Validator::new().validate_all(&wasm)?;
    assert!(import_names(&wasm)?.is_empty());

    Ok(())
}
