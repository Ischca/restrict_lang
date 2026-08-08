use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

fn compile_to_wat(source: &str) -> Result<String, String> {
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    let mut codegen = WasmCodeGen::new();
    codegen
        .generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

fn assert_valid_wat(name: &str, wat: &str) {
    let wasm = wat::parse_str(wat).unwrap_or_else(|err| {
        panic!("{name} generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("{name} generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn exported_record_is_source_level_only_and_emits_no_wasm_export() {
    let source = r#"
export record ReleaseSlice {
    score: Int32
}

fun main: () -> Int32 = {
    1
}
"#;

    let wat =
        compile_to_wat(source).expect("exported record should compile as source-level export");
    assert_valid_wat("exported_record_source_only", &wat);
    assert!(
        wat.contains("source export record ReleaseSlice has no direct Wasm export"),
        "WAT should document that record export is source-level only:\n{wat}"
    );
    assert!(
        !wat.contains("(export \"ReleaseSlice\""),
        "record export must not imply a host-visible Wasm ABI:\n{wat}"
    );
}
