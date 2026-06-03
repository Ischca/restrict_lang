use restrict_lang::ast::{
    BlockExpr, Expr, FieldDecl, FieldInit, FunDecl, Program, PrototypeCloneExpr, RecordDecl,
    RecordLit, TopDecl, Type,
};
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

fn int32() -> Type {
    Type::Named("Int32".to_string())
}

fn record_decl(name: &str, fields: Vec<(&str, Type)>) -> TopDecl {
    TopDecl::Record(RecordDecl {
        name: name.to_string(),
        type_params: Vec::new(),
        temporal_constraints: Vec::new(),
        fields: fields
            .into_iter()
            .map(|(name, ty)| FieldDecl {
                name: name.to_string(),
                ty,
            })
            .collect(),
        frozen: false,
        sealed: false,
        parent_hash: None,
    })
}

#[test]
fn freeze_uses_registered_record_layout_size() {
    let source = r#"
record Snapshot {
    a: Float64,
    b: Float64,
    c: Float64
}

fun main: () -> Float64 = {
    val base = Snapshot {
        a: 1.0,
        b: 2.0,
        c: 3.0
    };
    val frozen = base freeze;
    frozen.c
}
"#;

    let wat = compile_to_wat(source).expect("freeze should compile");

    assert!(
        wat.contains("i32.const 24 ;; frozen record size"),
        "freeze should allocate the registered 24-byte record layout:\n{wat}"
    );
    assert!(
        !wat.contains("i32.const 20 ;; record size"),
        "freeze should not use the old hardcoded 20-byte placeholder:\n{wat}"
    );
    assert!(
        !wat.contains("frozen flag"),
        "freeze codegen should not write metadata into field storage:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("freeze generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("freeze generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn generic_record_clone_update_generates_valid_wat() {
    let source = r#"
record Box<T> {
    value: T
}

fun main: () -> Int32 = {
    val base: Box<Int32> = Box { value: 1 };
    val updated = base.clone { value: 2 };
    updated.value
}
"#;

    let wat = compile_to_wat(source).expect("generic record clone should compile");
    assert!(
        wat.contains("i32.store"),
        "clone update should store the instantiated Int32 field:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic record clone generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic record clone generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn generic_record_freeze_generates_valid_wat() {
    let source = r#"
record Box<T> {
    value: T
}

fun main: () -> Int32 = {
    val base: Box<Int32> = Box { value: 7 };
    val frozen = base freeze;
    frozen.value
}
"#;

    let wat = compile_to_wat(source).expect("generic record freeze should compile");
    assert!(
        wat.contains(";; Freeze Box by copying record layout"),
        "freeze should use the underlying generic record layout:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("generic record freeze generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("generic record freeze generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn deeply_nested_record_literal_generates_enough_temporaries() {
    let source = r#"
record N0 { value: Int32 }
record N1 { child: N0 }
record N2 { child: N1 }
record N3 { child: N2 }
record N4 { child: N3 }
record N5 { child: N4 }
record N6 { child: N5 }
record N7 { child: N6 }
record N8 { child: N7 }

fun main: () -> Int32 = {
    val root = N8 {
        child: N7 {
            child: N6 {
                child: N5 {
                    child: N4 {
                        child: N3 {
                            child: N2 {
                                child: N1 {
                                    child: N0 { value: 1 }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
    root.child.child.child.child.child.child.child.child.value
}
"#;

    let wat = compile_to_wat(source).expect("deep nested record literal should compile");
    assert!(
        wat.contains("(local $record_tmp_8 i32)"),
        "codegen should declare enough record temporaries for actual nesting:\n{wat}"
    );

    let wasm = wat::parse_str(&wat).unwrap_or_else(|err| {
        panic!("deep nested record literal generated invalid WAT: {err}\n\n{wat}");
    });
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .unwrap_or_else(|err| {
            panic!("deep nested record literal generated invalid Wasm binary: {err}\n\n{wat}");
        });
}

#[test]
fn prototype_clone_codegen_rejects_placeholder_identity_metadata() {
    let program = Program {
        imports: Vec::new(),
        declarations: vec![
            record_decl("Base", vec![("id", int32())]),
            TopDecl::Function(FunDecl {
                name: "main".to_string(),
                is_async: false,
                type_params: Vec::new(),
                temporal_constraints: Vec::new(),
                params: Vec::new(),
                return_type: Some(Type::Named("Base".to_string())),
                body: BlockExpr {
                    statements: Vec::new(),
                    expr: Some(Box::new(Expr::PrototypeClone(PrototypeCloneExpr {
                        base: "Base".to_string(),
                        updates: RecordLit {
                            name: "Base".to_string(),
                            fields: vec![FieldInit::Field {
                                name: "id".to_string(),
                                value: Box::new(Expr::IntLit(7)),
                            }],
                        },
                        freeze_immediately: false,
                        sealed: false,
                    }))),
                },
            }),
        ],
    };

    let mut codegen = WasmCodeGen::new();
    let err = codegen
        .generate(&program)
        .expect_err("prototype clone must fail instead of emitting placeholder hashes");
    let message = err.to_string();

    assert!(
        message.contains("Unsupported feature: prototype clone for 'Base'"),
        "error should identify unsupported prototype clone codegen, got: {message}"
    );
    assert!(
        message.contains("real prototype identity metadata"),
        "error should explain why placeholder metadata is rejected, got: {message}"
    );
}
