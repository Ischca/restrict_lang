use restrict_lang::test_framework::{TestCase, run_test};

#[test]
fn test_simple_mutable_binding() {
    let test = TestCase {
        name: "simple_mutable_binding",
        input: "fun test = { mut val x = 5; x }",
        expected_ast: Some("mutable: true"),
        expected_type_error: None,
        expected_wat: Some("local.get $x"),
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_mutable_reassignment() {
    let test = TestCase {
        name: "mutable_reassignment",
        input: r#"fun test = {
            mut val x = 5;
            x = 10;
            x
        }"#,
        expected_ast: Some("mutable: true"),
        expected_type_error: None,
        expected_wat: Some("local.set $x"),
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_immutable_reassignment_error() {
    let test = TestCase {
        name: "immutable_reassignment_error",
        input: r#"fun test = {
            val x = 5;
            x = 10;
            x
        }"#,
        expected_ast: None,
        expected_type_error: Some("ImmutableReassignment"),
        expected_wat: None,
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_mutable_with_arithmetic() {
    let test = TestCase {
        name: "mutable_with_arithmetic",
        input: r#"fun test = {
            mut val x = 5;
            x = x + 1;
            x
        }"#,
        expected_ast: Some("mutable: true"),
        expected_type_error: None,
        expected_wat: Some("i32.add"),
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_while_with_mutable() {
    let test = TestCase {
        name: "while_with_mutable",
        input: r#"fun test = {
            mut val x = 0;
            mut val sum = 0;
            (x < 10) while {
                sum = sum + x;
                x = x + 1
            };
            sum
        }"#,
        expected_ast: Some("While"),
        expected_type_error: None,
        expected_wat: Some("(loop $while_loop"),
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_mutable_parameter_reassignment() {
    let test = TestCase {
        name: "mutable_parameter_reassignment",
        input: r#"fun add_one = x:Int {
            mut val y = x;
            y = y + 1;
            y
        }"#,
        expected_ast: Some("mutable: true"),
        expected_type_error: None,
        expected_wat: Some("local.get $x"),
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_multiple_reassignments() {
    let test = TestCase {
        name: "multiple_reassignments",
        input: r#"fun test = {
            mut val x = 1;
            x = 2;
            x = 3;
            x = 4;
            x
        }"#,
        expected_ast: Some("mutable: true"),
        expected_type_error: None,
        expected_wat: Some("local.set $x"),
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_affine_with_mutable() {
    let test = TestCase {
        name: "affine_with_mutable",
        input: r#"fun test = {
            val y = 5;
            mut val x = y;
            x = x + 1;
            x
        }"#,
        expected_ast: Some("mutable: true"),
        expected_type_error: None,
        expected_wat: None,
    };
    
    run_test(&test).unwrap();
}

#[test]
fn test_mutable_record_field() {
    let test = TestCase {
        name: "mutable_record_field",
        input: r#"
        record Point { x: Int y: Int }
        
        fun test = {
            val p = Point { x = 10, y = 20 };
            mut val x = p.x;
            x = x + 1;
            x
        }"#,
        expected_ast: Some("mutable: true"),
        expected_type_error: None,
        expected_wat: None,
    };
    
    run_test(&test).unwrap();
}