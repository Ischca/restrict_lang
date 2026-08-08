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

#[test]
fn bare_osv_output_stops_before_a_non_callable_value() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun bare_osv_output: () -> () = {
    "Hello, " print
    "Restrict" println
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let bare_osv_output = instance.get_typed_func::<(), ()>(&store, "bare_osv_output")?;

    bare_osv_output.call(&mut store, ())?;

    assert_eq!(store.data().stdout, b"Hello, Restrict\n");
    Ok(())
}

#[test]
fn scoped_collection_clauses_execute_through_existing_lambdas(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun scoped_collection_total: () -> Int32 = {
    val values = [20, 21]
    val shifted = values map {
        it + 1
    }
    (shifted, 0) fold { |total, value|
        total + value
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let scoped_collection_total =
        instance.get_typed_func::<(), i32>(&store, "scoped_collection_total")?;

    assert_eq!(scoped_collection_total.call(&mut store, ())?, 43);
    Ok(())
}

#[test]
fn display_formats_all_builtin_scalar_adoptions() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun emit_i32: (value: Int32) -> () = { value |> println }
export fun emit_i64: (value: Int64) -> () = { value |> println }
export fun emit_f64: (value: Float64) -> () = { value |> println }
export fun emit_other_scalars: () -> () = {
    true |> println;
    false |> println;
    'A' |> println;
    '界' |> println;
    '🙂' |> println;
    () |> println
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let emit_i32 = instance.get_typed_func::<i32, ()>(&store, "emit_i32")?;
    let emit_i64 = instance.get_typed_func::<i64, ()>(&store, "emit_i64")?;
    let emit_f64 = instance.get_typed_func::<f64, ()>(&store, "emit_f64")?;
    let emit_other_scalars = instance.get_typed_func::<(), ()>(&store, "emit_other_scalars")?;

    emit_i32.call(&mut store, i32::MIN)?;
    emit_i64.call(&mut store, i64::MIN)?;
    emit_f64.call(&mut store, 3.14)?;
    emit_f64.call(&mut store, -3.14)?;
    emit_f64.call(&mut store, -0.0)?;
    emit_f64.call(&mut store, f64::NAN)?;
    emit_f64.call(&mut store, f64::INFINITY)?;
    emit_f64.call(&mut store, f64::NEG_INFINITY)?;
    emit_other_scalars.call(&mut store, ())?;

    assert_eq!(
        store.data().stdout,
        b"-2147483648\n-9223372036854775808\n3.14\n-3.14\n0.00\nNaN\nInfinity\n-Infinity\ntrue\nfalse\nA\n\xe7\x95\x8c\n\xf0\x9f\x99\x82\n()\n"
    );
    Ok(())
}

#[test]
fn custom_forms_dispatch_direct_pipe_and_through_generics() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
form Labeled {
    fun label: (self: Self) -> String
}

record Widget {
    text: String
}

Widget takes Labeled {
    fun label: (self: Widget) -> String = { self.text }
}

fun render: <T of Labeled>(value: T) -> String = { value |> label }

export fun form_runtime_smoke: () -> () = {
    val direct = Widget { text: "direct" };
    (direct) label |> println;
    val piped = Widget { text: "pipe" };
    piped |> label |> println;
    Widget { text: "generic form" } |> render |> println
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let form_runtime_smoke = instance.get_typed_func::<(), ()>(&store, "form_runtime_smoke")?;
    form_runtime_smoke.call(&mut store, ())?;

    assert_eq!(store.data().stdout, b"direct\npipe\ngeneric form\n");
    Ok(())
}

#[test]
fn custom_form_methods_support_multiple_osv_arguments() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
form Addable {
    fun add: (self: Self, extra: Int32) -> Int32
}

record Counter {
    value: Int32
}

Counter takes Addable {
    fun add: (self: Counter, extra: Int32) -> Int32 = {
        self.value + extra
    }
}

fun add_generic: <T of Addable>(value: T, extra: Int32) -> Int32 = {
    (value, extra) add
}

export fun multi_arg_form_smoke: () -> Int32 = {
    (Counter { value: 40 }, 2) add_generic
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let smoke = instance.get_typed_func::<(), i32>(&store, "multi_arg_form_smoke")?;
    assert_eq!(smoke.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn form_method_symbols_do_not_collide_across_component_boundaries(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
form A {
    fun c_d: (self: Self) -> Int32
    fun d: (self: Self) -> Int32
}

record B {
    value: Int32
}

record B_c {
    value: Int32
}

B takes A {
    fun c_d: (self: B) -> Int32 = { self.value }
    fun d: (self: B) -> Int32 = { self.value }
}

B_c takes A {
    fun c_d: (self: B_c) -> Int32 = { self.value }
    fun d: (self: B_c) -> Int32 = { self.value }
}

fun __restrict_form_41_for_42_635f64: (value: Int32) -> Int32 = {
    value + 1
}

export fun form_symbol_collision_smoke: () -> Int32 = {
    val left = B { value: 10 } |> c_d;
    val right = B_c { value: 20 } |> d;
    val source_function = 11 |> __restrict_form_41_for_42_635f64;
    left + right + source_function
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let smoke = instance.get_typed_func::<(), i32>(&store, "form_symbol_collision_smoke")?;
    assert_eq!(smoke.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn local_callables_shadow_display_intrinsic_spellings() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
export fun display_shadow_smoke: () -> Int32 = {
    val display: Int32 -> Int32 = |value| value + 1;
    val print: Int32 -> Int32 = |value| value + 1;
    val println: Int32 -> Int32 = |value| value + 1;
    val called = (39) display;
    val piped = called |> print;
    piped |> (println)
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let smoke = instance.get_typed_func::<(), i32>(&store, "display_shadow_smoke")?;
    assert_eq!(smoke.call(&mut store, ())?, 42);
    assert!(store.data().stdout.is_empty());
    Ok(())
}

#[test]
fn local_display_callable_result_feeds_form_dispatch() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
form Readable {
    fun read: (self: Self) -> Int32
}

record Reading {
    value: Int32
}

Reading takes Readable {
    fun read: (self: Reading) -> Int32 = {
        self.value
    }
}

export fun composed_display_shadow_smoke: () -> Int32 = {
    val display: Int32 -> Reading = |value| Reading { value: value + 1 };
    41 |> display |> read
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let smoke = instance.get_typed_func::<(), i32>(&store, "composed_display_shadow_smoke")?;
    assert_eq!(smoke.call(&mut store, ())?, 42);
    assert!(store.data().stdout.is_empty());
    Ok(())
}

#[test]
fn form_methods_precede_same_named_builtins_after_lexical_callables(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
form Mappable {
    fun map: (self: Self) -> Int32
}

form Identified {
    fun identity: (self: Self) -> Int32
}

record FormBox {
    value: Int32
}

record IdentityBox {
    value: Int32
}

record ImplBox {
    value: Int32
}

FormBox takes Mappable {
    fun map: (self: FormBox) -> Int32 = { self.value }
}

IdentityBox takes Identified {
    fun identity: (self: IdentityBox) -> Int32 = { self.value }
}

impl ImplBox {
    fun map: (self: ImplBox) -> Int32 = { self.value }
    fun identity: (self: ImplBox) -> Int32 = { self.value + 100 }
}

fun generic_map: <T of Mappable>(value: T) -> Int32 = {
    value |> map
}

fun expected_identity_collision: () -> Int32 = {
    IdentityBox { value: 20 } |> identity
}

export fun receiver_precedence_smoke: () -> Int32 = {
    val direct = (FormBox { value: 1 }) map;
    val pipe_ident = FormBox { value: 2 } |> map;
    val pipe_expr = FormBox { value: 3 } |> (map);
    val generic = FormBox { value: 4 } |> generic_map;
    val ordinary_impl = (ImplBox { value: 5 }) map;
    val global_identity = ImplBox { value: 0 } |> identity;
    val identity_value = global_identity.value;
    val map: Int32 -> Int32 = |value| value + 1;
    val lexical = 6 |> map;
    val identity_method = () expected_identity_collision;
    direct + pipe_ident + pipe_expr + generic + ordinary_impl + identity_value + lexical + identity_method
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let smoke = instance.get_typed_func::<(), i32>(&store, "receiver_precedence_smoke")?;
    assert_eq!(smoke.call(&mut store, ())?, 42);
    Ok(())
}

#[test]
fn lexical_callable_aliases_keep_precedence_and_stack_abis(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun generic_float: <T>(value: T) -> Float64 = { 1.5 }
fun generic_int: <T>(value: T) -> Int32 = { 1 }

form AliasSurface {
    fun form_pick: (self: Self) -> Int32
    fun stored_pick: (self: Self) -> Int32
    fun future_pick: (self: Self) -> Int32
    fun score: (self: Self) -> Int64
}

record AliasBox {
    value: Int32
    wide: Int64
}

AliasBox takes AliasSurface {
    fun form_pick: (self: AliasBox) -> Int32 = { self.value + 100 }
    fun stored_pick: (self: AliasBox) -> Int32 = { self.value + 200 }
    fun future_pick: (self: AliasBox) -> Int32 = { 7 }
    fun score: (self: AliasBox) -> Int64 = { self.wide }
}

impl AliasBox {
    fun impl_pick: (self: AliasBox) -> Int32 = { self.value + 300 }
}

fun global_pick: (value: AliasBox) -> Int32 = { value.value + 400 }
fun score: () -> () = { () }
fun consume: (action: Int32 -> Int32) -> () = { () }

export fun generic_direct_normal: () -> Float64 = {
    val form_pick = generic_float;
    ((AliasBox { value: 1, wide: 1 }) form_pick) + 0.0
}

export fun generic_direct_expected: () -> Float64 = {
    val impl_pick = generic_float;
    val result = (AliasBox { value: 1, wide: 1 }) impl_pick;
    result
}

export fun generic_pipe_ident: () -> Float64 = {
    val global_pick = generic_float;
    AliasBox { value: 1, wide: 1 } |> global_pick
}

export fun generic_pipe_expr: () -> Float64 = {
    val display = generic_float;
    41 |> (display)
}

export fun generic_identity_shadow: () -> Float64 = {
    val identity = generic_float;
    41 |> identity
}

export fun generic_stored_pipe: () -> Float64 = {
    val stored_pick = generic_float;
    val result = AliasBox { value: 1, wide: 1 } |> stored_pick;
    result
}

export fun deferred_direct_normal: () -> Float64 = {
    val form_pick = true then { |value| 2.5 } else { |value| 3.5 };
    ((AliasBox { value: 1, wide: 1 }) form_pick) + 0.0
}

export fun deferred_direct_expected: () -> Float64 = {
    val impl_pick = true then { |value| 2.5 } else { |value| 3.5 };
    val result = (AliasBox { value: 1, wide: 1 }) impl_pick;
    result
}

export fun deferred_pipe_ident: () -> Float64 = {
    val global_pick = true then { |value| 2.5 } else { |value| 3.5 };
    AliasBox { value: 1, wide: 1 } |> global_pick
}

export fun deferred_pipe_expr: () -> Float64 = {
    val display = true then { |value| 2.5 } else { |value| 3.5 };
    41 |> (display)
}

export fun deferred_stored_pipe: () -> Float64 = {
    val stored_pick = true then { |value| 2.5 } else { |value| 3.5 };
    val result = AliasBox { value: 1, wide: 1 } |> stored_pick;
    result
}

export fun future_alias_does_not_shadow: () -> Int32 = {
    val before = AliasBox { value: 1, wide: 1 } |> future_pick;
    val future_pick = generic_int;
    val after = AliasBox { value: 1, wide: 1 } |> future_pick;
    before * 10 + after
}

export fun local_print_statement: () -> Int32 = {
    val print = generic_float;
    (41) print;
    42
}

export fun form_result_statement: () -> Int32 = {
    (AliasBox { value: 1, wide: 9 }) score;
    42
}

export fun higher_order_unit_statement: () -> Int32 = {
    val action: Int32 -> Int32 = |value: Int32| value;
    (action) consume;
    42
}
"#;

    let (mut store, instance) = instantiate(source)?;
    for name in [
        "generic_direct_normal",
        "generic_direct_expected",
        "generic_pipe_ident",
        "generic_pipe_expr",
        "generic_identity_shadow",
        "generic_stored_pipe",
    ] {
        let function = instance.get_typed_func::<(), f64>(&store, name)?;
        assert_eq!(function.call(&mut store, ())?, 1.5, "{name}");
    }
    for name in [
        "deferred_direct_normal",
        "deferred_direct_expected",
        "deferred_pipe_ident",
        "deferred_pipe_expr",
        "deferred_stored_pipe",
    ] {
        let function = instance.get_typed_func::<(), f64>(&store, name)?;
        assert_eq!(function.call(&mut store, ())?, 2.5, "{name}");
    }

    let future = instance.get_typed_func::<(), i32>(&store, "future_alias_does_not_shadow")?;
    assert_eq!(future.call(&mut store, ())?, 71);
    for name in [
        "local_print_statement",
        "form_result_statement",
        "higher_order_unit_statement",
    ] {
        let function = instance.get_typed_func::<(), i32>(&store, name)?;
        assert_eq!(function.call(&mut store, ())?, 42, "{name}");
    }
    assert!(store.data().stdout.is_empty());
    Ok(())
}

#[test]
fn lexical_callable_visibility_is_source_ordered_inside_lambdas_and_map(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
fun generic_float: <T>(value: T) -> Float64 = { 1.5 }
fun convert: (value: Int32) -> Int64 = { 10000000000 }

form Runnable {
    fun run: (self: Self) -> Float64
}

record CaptureBox {
    value: Int32
}

CaptureBox takes Runnable {
    fun run: (self: CaptureBox) -> Float64 = { 4.5 }
}

export fun future_lambda_binding_uses_form: () -> Float64 = {
    val action: CaptureBox -> Float64 = |box: CaptureBox| box |> run;
    val run: CaptureBox -> Int32 = |box: CaptureBox| 99;
    CaptureBox { value: 1 } |> action
}

export fun generic_alias_inside_lambda: () -> Float64 = {
    val choose = generic_float;
    val action: CaptureBox -> Float64 = |box: CaptureBox| box |> choose;
    CaptureBox { value: 1 } |> action
}

export fun deferred_alias_inside_lambda: (flag: Boolean) -> Float64 = {
    val choose = flag then { |value| 2.5 } else { |value| 3.5 };
    val action: CaptureBox -> Float64 = |box: CaptureBox| box |> choose;
    CaptureBox { value: 1 } |> action
}

export fun future_map_binding_uses_global: () -> Int64 = {
    with Arena {
        val numbers = [1, 2];
        val converted = (numbers, convert) map;
        val first = (converted, 0) list_get;
        val convert: Float64 -> Float64 = |value: Float64| value + 0.5;
        first
    }
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let future_lambda =
        instance.get_typed_func::<(), f64>(&store, "future_lambda_binding_uses_form")?;
    assert_eq!(future_lambda.call(&mut store, ())?, 4.5);

    let generic_alias =
        instance.get_typed_func::<(), f64>(&store, "generic_alias_inside_lambda")?;
    assert_eq!(generic_alias.call(&mut store, ())?, 1.5);

    let deferred_alias =
        instance.get_typed_func::<i32, f64>(&store, "deferred_alias_inside_lambda")?;
    assert_eq!(deferred_alias.call(&mut store, 1)?, 2.5);
    assert_eq!(deferred_alias.call(&mut store, 0)?, 3.5);

    let future_map =
        instance.get_typed_func::<(), i64>(&store, "future_map_binding_uses_global")?;
    assert_eq!(future_map.call(&mut store, ())?, 10000000000);
    Ok(())
}

#[test]
fn display_intrinsics_dispatch_inside_generic_functions() -> Result<(), Box<dyn std::error::Error>>
{
    let source = r#"
record Widget {
    text: String
}

Widget takes Display {
    fun display: (self: Widget) -> String = { self.text }
}

fun display_text: <T of Display>(value: T) -> String = { value |> display }
fun emit: <T of Display>(value: T) -> () = { value |> println }
fun emit_inline: <T of Display>(value: T) -> () = { value |> print }

export fun generic_display_smoke: () -> () = {
    42 |> display_text |> println;
    "generic println" |> emit;
    Widget { text: "record display" } |> emit;
    7 |> emit_inline;
    "!" |> println
}
"#;

    let (mut store, instance) = instantiate(source)?;
    let generic_display_smoke =
        instance.get_typed_func::<(), ()>(&store, "generic_display_smoke")?;
    generic_display_smoke.call(&mut store, ())?;

    assert_eq!(
        store.data().stdout,
        b"42\ngeneric println\nrecord display\n7!\n"
    );
    Ok(())
}
