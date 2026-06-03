use restrict_lang::{generate, parse_program, TypeChecker};

fn compile(source: &str) -> Result<String, String> {
    // Parse
    let (remaining, ast) = parse_program(source).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker
        .check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;

    // Generate code
    generate(&ast).map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_array_literal() {
    let source = r#"
        fun main: () = {
            val arr: Array<Int32, 5> = [1, 2, 3, 4, 5];
            arr
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    // Check that array allocation happens
    assert!(wat.contains("array size"));
    assert!(wat.contains("call $allocate"));
    assert!(wat.contains("i32.const 28 ;; array size"));
    assert!(wat.contains("i32.const 5 ;; array length"));
    assert!(wat.contains("i32.const 4 ;; array element size"));
    assert!(wat.contains("i32.const 24 ;; offset to element 4"));
}

#[test]
fn test_contextless_empty_bracket_literal_requires_expected_type() {
    let source = r#"
        fun main: () = {
            val empty = [];
            empty
        }
    "#;

    let result = compile(source);
    let err = result.expect_err("contextless empty collection should be rejected");
    assert!(
        err.contains("empty list requires an expected List type")
            || err.contains("unresolved type"),
        "error should explain missing collection context, got: {}",
        err
    );
}

#[test]
fn test_source_array_type_requires_explicit_length() {
    let source = r#"
        fun main: () = {
            val arr: Array<Int32> = [1, 2, 3];
            arr
        }
    "#;

    let result = compile(source);
    let err = result.expect_err("source Array<T> should require an explicit length");
    assert!(
        err.contains("Array type requires explicit length") && err.contains("Array<T, N>"),
        "error should explain the public Array<T, N> surface, got: {}",
        err
    );
}

#[test]
fn test_empty_array_infers_from_array_get_return_context() {
    let source = r#"
        fun main: () -> Int32 = {
            ([], 0) array_get
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    assert!(wat.contains("call $array_get"));
    assert!(wat.contains("i32.const 8 ;; array size"));
    assert!(wat.contains("i32.const 0 ;; array length"));
}

#[test]
fn test_non_empty_local_array_infers_from_later_array_get() {
    let source = r#"
        fun main: () -> Int32 = {
            val arr = [10, 20, 30];
            (arr, 1) array_get
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    assert!(
        wat.contains("call $array_get"),
        "WAT should route later-use inference through array_get:\n{wat}"
    );
}

#[test]
fn test_public_zero_length_array_is_not_wildcard() {
    let source = r#"
        fun main: () = {
            val arr: Array<Int32, 0> = [10];
            arr
        }
    "#;

    let result = compile(source);
    let err = result.expect_err("public Array<T, 0> should be a real zero-length array");
    assert!(
        err.contains("Array<Int32, 0>") && err.contains("Array<Int32, 1>"),
        "error should compare concrete public array lengths, got: {}",
        err
    );
}

#[test]
fn test_array_get() {
    let source = r#"
        fun main: () = {
            with Arena {
                val arr: Array<Int32, 5> = [10, 20, 30, 40, 50];
                val third = (arr, 2) array_get;
                third
            }
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    // Check that array_get function is called
    assert!(wat.contains("call $array_get"));
}

#[test]
fn test_array_set() {
    let source = r#"
        fun main: () = {
            with Arena {
                mut val arr: Array<Int32, 5> = [10, 20, 30, 40, 50];
                (arr, 2, 35) array_set;
                val third = (arr, 2) array_get;
                third
            }
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    // Check that array_set function is called
    assert!(wat.contains("call $array_set"));
}

#[test]
fn test_float64_array_set_uses_specialized_abi() {
    let source = r#"
        fun main: () -> Float64 = {
            with Arena {
                mut val arr: Array<Float64, 3> = [1.5, 2.5, 3.5];
                (arr, 1, 4.5) array_set;
                (arr, 1) array_get
            }
        }
    "#;

    let wat = compile(source).expect("Float64 array set/get should compile");

    assert!(wat.contains("call $array_set_f64"));
    assert!(wat.contains("call $array_get_f64"));
}

#[test]
fn test_int64_array_set_uses_specialized_abi() {
    let source = r#"
        fun main: () -> Int64 = {
            with Arena {
                mut val arr: Array<Int64, 3> = [10000000000, 20000000000, 30000000000];
                (arr, 1, 40000000000) array_set;
                (arr, 1) array_get
            }
        }
    "#;

    let wat = compile(source).expect("Int64 array set/get should compile");

    assert!(wat.contains("call $array_set_i64"));
    assert!(wat.contains("call $array_get_i64"));
}

#[test]
fn test_array_vs_list() {
    let source = r#"
        fun main: () = {
            with Arena {
                val list = [1, 2, 3];      // List literal
                val arr: Array<Int32, 3> = [1, 2, 3];     // Array by expected type
                
                val list_len = list |> list_length;
                val arr_first = (arr, 0) array_get;
                
                list_len + arr_first
            }
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    // Check that both list and array literals are generated
    assert!(wat.contains("list size"));
    assert!(wat.contains("array size"));
}

#[test]
fn test_deprecated_pipe_array_literal_is_rejected() {
    let source = r#"
        fun main: () = {
            [|1, 2, 3|]
        }
    "#;

    let result = compile(source);
    let err = result.expect_err("deprecated [| |] array syntax should be rejected");
    assert!(
        err.contains("Parse error") || err.contains("Unparsed input"),
        "old array syntax should fail during parsing, got: {}",
        err
    );
}

#[test]
fn test_spec_array_type_annotation_uses_bracket_literal() {
    let source = r#"
        fun main: () -> Int32 = {
            with Arena {
                val arr: Array<Int32, 3> = [10, 20, 30];
                (arr, 1) array_get
            }
        }
    "#;

    let result = compile(source);
    if let Err(e) = &result {
        println!("Error: {}", e);
    }
    assert!(result.is_ok());
    let wat = result.unwrap();

    assert!(wat.contains("array size"));
    assert!(wat.contains("call $array_get"));
}

#[test]
fn test_spec_array_length_mismatch_is_rejected() {
    let source = r#"
        fun main: () = {
            val arr: Array<Int32, 2> = [10, 20, 30];
            arr
        }
    "#;

    let result = compile(source);
    let err = result.expect_err("array annotation length should be enforced");
    assert!(
        err.contains("Array<Int32, 2>")
            && err.contains("Array<Int32, 3>")
            && !err.contains("Array<Int32, _>"),
        "error should explain array length mismatch, got: {}",
        err
    );
}
