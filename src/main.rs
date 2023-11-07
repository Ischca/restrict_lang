use inkwell::{
    context::Context,
    module::Module,
    types::BasicTypeEnum,
    values::{BasicValueEnum, FunctionValue},
    AddressSpace, OptimizationLevel,
};
use nom::{
    bytes::complete::tag,
    character::complete::{alphanumeric1, char, multispace0},
    combinator::map_res,
    sequence::{delimited, tuple},
    IResult,
};
use std::{
    env,
    fs::File,
    io::{self, Read},
    path::Path,
};

// `is_not`をimport
use nom::bytes::complete::is_not;

fn main() {
    let args: Vec<String> = env::args().collect();
    let input_filename = &args[1];
    let output_filename = &args[2];

    let mut input_file = File::open(input_filename).expect("Failed to open input file.");
    let mut restrict_code = String::new();
    input_file
        .read_to_string(&mut restrict_code)
        .expect("Failed to read input file.");

    let (_, parsed_code) = parse_restrict_code(&restrict_code).unwrap();
    let context = Context::create();
    let module = compile_to_llvm_ir(&context, &parsed_code);
    let target_triple = &inkwell::targets::TargetTriple::create("x86_64-pc-linux-gnu");
    let target = inkwell::targets::Target::from_name("x86-64").unwrap();
    let target_machine = target
        .create_target_machine(
            &target_triple,
            "generic",
            "",
            OptimizationLevel::Default,
            inkwell::targets::RelocMode::Default,
            inkwell::targets::CodeModel::Default,
        )
        .expect("Failed to create target machine.");

    if let Err(err) = target_machine.write_to_file(
        &module,
        inkwell::targets::FileType::Object,
        &Path::new(&output_filename),
    ) {
        eprintln!("Failed to write object file: {}", err);
        return;
    }
}

fn parse_restrict_code(input: &str) -> IResult<&str, String> {
    // `is_not`を使用して、引用符内の任意の文字を受け入れるように修正しました。
    delimited(
        tag("\""),
        is_not("\""),
        tag("\" print"),
    )(input)
    .map(|(next_input, result)| (next_input, result.to_string()))
}

fn compile_to_llvm_ir<'ctx>(
    context: &'ctx Context,
    restrict_code: &str,
) -> Module<'ctx> {
    let module = context.create_module("main");
    let builder = context.create_builder();

    let i8_type = context.i8_type(); // 追加
    let void_type = context.void_type();
    let putchar_type = i8_type.fn_type(&[i8_type.into()], false);

    // `putchar`関数を外部関数として宣言します。
    let putchar_func = module.add_function("putchar", putchar_type, None);

    // main関数の型を定義します。
    let fn_type = void_type.fn_type(&[], false);
    let main_func = module.add_function("main", fn_type, None);

    // main関数のエントリーブロックを作成し、ビルダーを配置します。
    let entry = context.append_basic_block(main_func, "entry");
    builder.position_at_end(entry);

    // 文字列リテラルをグローバルとして追加します。
    let string = format!("{}\0", restrict_code); // Null終端を追加
    let global_string = module.add_global(i8_type.array_type(string.len() as u32), None, "str");
    global_string.set_initializer(&context.const_string(string.as_bytes(), false));

    // 文字列の先頭アドレスを取得します。
    let zero = context.i32_type().const_int(0, false);
    let ptr = unsafe {
        builder.build_gep(
            global_string.as_pointer_value(),
            &[zero, zero],
            "str_ptr",
        )
    };

    // 各文字を`putchar`関数に渡して呼び出します。
    for i in 0..(string.len() - 1) { // Null終端を含まないようにする
        let index = context.i32_type().const_int(i as u64, false);
        let ch_ptr = unsafe {
            builder.build_gep(ptr, &[index], "ch_ptr")
        };
        let ch = builder.build_load(ch_ptr, "ch").into_int_value();
        builder.build_call(putchar_func, &[ch.into()], "putchar_call");
    }

    builder.build_return(None);

    module
}