use restrict_lang::*;

// ---- Helpers ----

fn parse_ok(input: &str) -> Program {
    match parse_program(input) {
        Ok((_, program)) => program,
        Err(e) => panic!("Parse failed: {:?}", e),
    }
}

fn type_check(input: &str) -> Result<(), TypeError> {
    let program = parse_ok(input);
    let mut checker = TypeChecker::new();
    checker.check_program(&program)
}

// ========================================================================
// 1. Parsing tests
// ========================================================================

#[test]
fn test_parse_simple_form_declaration() {
    let input = r#"
        form Showable {
            show: (self) -> String
        }
    "#;
    let program = parse_ok(input);
    assert_eq!(program.declarations.len(), 1);
    match &program.declarations[0] {
        TopDecl::Form(form) => {
            assert_eq!(form.name, "Showable");
            assert!(form.type_params.is_empty());
            assert!(form.associated_types.is_empty());
            assert_eq!(form.methods.len(), 1);
            assert_eq!(form.methods[0].name, "show");
        }
        other => panic!("Expected Form declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_form_with_type_params() {
    let input = r#"
        form Container<T> {
            append: (self, elem: T) -> Self
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[0] {
        TopDecl::Form(form) => {
            assert_eq!(form.name, "Container");
            assert_eq!(form.type_params.len(), 1);
            assert_eq!(form.type_params[0].name, "T");
        }
        other => panic!("Expected Form declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_form_with_associated_types() {
    let input = r#"
        form Functor<T> {
            type Mapped<U>
            fmap: (x: T) -> U
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[0] {
        TopDecl::Form(form) => {
            assert_eq!(form.associated_types.len(), 1);
            assert_eq!(form.associated_types[0].name, "Mapped");
            assert_eq!(form.associated_types[0].type_params.len(), 1);
            assert_eq!(form.associated_types[0].type_params[0].name, "U");
        }
        other => panic!("Expected Form declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_form_with_multiple_methods() {
    let input = r#"
        form Collection<T> {
            size: (x: Int32) -> Int32
            empty: () -> Int32
            append: (x: Int32, elem: T) -> Int32
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[0] {
        TopDecl::Form(form) => {
            assert_eq!(form.methods.len(), 3);
            assert_eq!(form.methods[0].name, "size");
            assert_eq!(form.methods[1].name, "empty");
            assert_eq!(form.methods[2].name, "append");
        }
        other => panic!("Expected Form declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_form_method_with_type_params() {
    let input = r#"
        form Container<T> {
            fold<U>: (x: T, init: U) -> U
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[0] {
        TopDecl::Form(form) => {
            assert_eq!(form.methods[0].name, "fold");
            assert_eq!(form.methods[0].type_params.len(), 1);
            assert_eq!(form.methods[0].type_params[0].name, "U");
        }
        other => panic!("Expected Form declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_form_method_with_return_type() {
    let input = r#"
        form Measurable {
            length: (x: Int32) -> Int32
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[0] {
        TopDecl::Form(form) => {
            assert!(form.methods[0].return_type.is_some());
        }
        other => panic!("Expected Form declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_takes_declaration() {
    let input = r#"
        form Showable {
            show: (x: Int32) -> String
        }

        MyType takes Showable {
            show = |x| { "hello" }
        }
    "#;
    let program = parse_ok(input);
    assert_eq!(program.declarations.len(), 2);
    match &program.declarations[1] {
        TopDecl::Takes(takes) => {
            assert_eq!(takes.type_name, "MyType");
            assert_eq!(takes.form_name, "Showable");
            assert_eq!(takes.method_impls.len(), 1);
            assert_eq!(takes.method_impls[0].name, "show");
        }
        other => panic!("Expected Takes declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_takes_with_type_params() {
    let input = r#"
        form Container<T> {
            append: (x: T) -> T
        }

        MyList<T> takes Container<T> {
            append = |x| { x }
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[1] {
        TopDecl::Takes(takes) => {
            assert_eq!(takes.type_name, "MyList");
            assert_eq!(takes.type_params.len(), 1);
            assert_eq!(takes.type_params[0].name, "T");
            assert_eq!(takes.form_name, "Container");
            assert_eq!(takes.form_type_args.len(), 1);
        }
        other => panic!("Expected Takes declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_takes_with_associated_type_impl() {
    let input = r#"
        form Functor<T> {
            type Mapped<U>
            fmap: (x: T) -> U
        }

        MyList<T> takes Functor<T> {
            type Mapped<U> = MyList<U>
            fmap = |x| { x }
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[1] {
        TopDecl::Takes(takes) => {
            assert_eq!(takes.associated_type_impls.len(), 1);
            assert_eq!(takes.associated_type_impls[0].name, "Mapped");
            assert_eq!(takes.associated_type_impls[0].type_params.len(), 1);
        }
        other => panic!("Expected Takes declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_of_constraint_in_type_param() {
    let input = r#"
        form Showable {
            show: (x: Int32) -> String
        }

        fun display<T of Showable>: (x: T) -> String = {
            "result"
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[1] {
        TopDecl::Function(fun) => {
            assert_eq!(fun.type_params.len(), 1);
            assert_eq!(fun.type_params[0].of_forms.len(), 1);
            assert_eq!(fun.type_params[0].of_forms[0], "Showable");
        }
        other => panic!("Expected Function declaration, got {:?}", other),
    }
}

#[test]
fn test_parse_multiple_of_constraints() {
    let input = r#"
        form Showable {
            show: (x: Int32) -> String
        }
        form Comparable {
            compare: (a: Int32, b: Int32) -> Int32
        }

        fun process<T of Showable + Comparable>: (x: T) -> String = {
            "result"
        }
    "#;
    let program = parse_ok(input);
    match &program.declarations[2] {
        TopDecl::Function(fun) => {
            assert_eq!(fun.type_params[0].of_forms.len(), 2);
            assert_eq!(fun.type_params[0].of_forms[0], "Showable");
            assert_eq!(fun.type_params[0].of_forms[1], "Comparable");
        }
        other => panic!("Expected Function declaration, got {:?}", other),
    }
}

// ========================================================================
// 2. Type checking tests
// ========================================================================

#[test]
fn test_typecheck_form_declaration_registers() {
    // Use only known types (Int32, String) to avoid UnknownType errors
    let input = r#"
        form Computable {
            compute: (x: Int32) -> Int32
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_form_with_associated_types_registers() {
    let input = r#"
        form Container<T> {
            type Mapped<U>
            fold<U>: (x: T, init: U) -> U
            append: (x: T, elem: T) -> T
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_form_with_multiple_known_type_methods() {
    let input = r#"
        form MathOps {
            add: (a: Int32, b: Int32) -> Int32
            multiply: (a: Int32, b: Int32) -> Int32
            describe: (x: Int32) -> String
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_duplicate_form_gives_error() {
    let input = r#"
        form Computable {
            compute: (x: Int32) -> Int32
        }
        form Computable {
            run: (x: Int32) -> Int32
        }
    "#;
    let result = type_check(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        TypeError::DuplicateForm(name) => {
            assert_eq!(name, "Computable");
        }
        other => panic!("Expected DuplicateForm error, got {:?}", other),
    }
}

#[test]
fn test_typecheck_takes_undefined_form_gives_error() {
    let input = r#"
        MyType takes NonExistentForm {
            foo = |x| { 42 }
        }
    "#;
    let result = type_check(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        TypeError::UndefinedForm(name) => {
            assert_eq!(name, "NonExistentForm");
        }
        other => panic!("Expected UndefinedForm error, got {:?}", other),
    }
}

#[test]
fn test_typecheck_takes_missing_method_gives_error() {
    let input = r#"
        form Ops {
            run: (x: Int32) -> Int32
            stop: (x: Int32) -> Int32
        }

        MyType takes Ops {
            run = |x| { 42 }
        }
    "#;
    let result = type_check(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        TypeError::MissingFormMethod { form, method } => {
            assert_eq!(form, "Ops");
            assert_eq!(method, "stop");
        }
        other => panic!("Expected MissingFormMethod error, got {:?}", other),
    }
}

#[test]
fn test_typecheck_takes_missing_associated_type_gives_error() {
    let input = r#"
        form Functor<T> {
            type Mapped<U>
            fmap: (x: T) -> T
        }

        MyList<T> takes Functor<T> {
            fmap = |x| { x }
        }
    "#;
    let result = type_check(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        TypeError::MissingAssociatedType { form, assoc_type } => {
            assert_eq!(form, "Functor");
            assert_eq!(assoc_type, "Mapped");
        }
        other => panic!("Expected MissingAssociatedType error, got {:?}", other),
    }
}

#[test]
fn test_typecheck_valid_takes_succeeds() {
    let input = r#"
        form Computable {
            compute: (x: Int32) -> Int32
        }

        MyType takes Computable {
            compute = |x| { 42 }
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_valid_takes_with_associated_type_succeeds() {
    let input = r#"
        form Functor<T> {
            type Mapped<U>
            fmap: (x: T) -> T
        }

        MyList<T> takes Functor<T> {
            type Mapped<U> = List<U>
            fmap = |x| { x }
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_multiple_takes_for_same_type() {
    let input = r#"
        form Computable {
            compute: (x: Int32) -> Int32
        }
        form Describable {
            describe: (x: Int32) -> String
        }

        MyType takes Computable {
            compute = |x| { 42 }
        }
        MyType takes Describable {
            describe = |x| { "hello" }
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_empty_form_succeeds() {
    let input = r#"
        form Marker {
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_takes_empty_form_succeeds() {
    let input = r#"
        form Marker {
        }

        MyType takes Marker {
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_takes_with_all_methods_provided() {
    let input = r#"
        form FullOps {
            add: (a: Int32, b: Int32) -> Int32
            sub: (a: Int32, b: Int32) -> Int32
            mul: (a: Int32, b: Int32) -> Int32
        }

        Calculator takes FullOps {
            add = |a, b| { 0 }
            sub = |a, b| { 0 }
            mul = |a, b| { 0 }
        }
    "#;
    assert!(type_check(input).is_ok());
}

// ========================================================================
// 3. Error message tests
// ========================================================================

#[test]
fn test_error_message_duplicate_form() {
    let input = r#"
        form Duplicated {
            run: (x: Int32) -> Int32
        }
        form Duplicated {
            execute: (x: Int32) -> Int32
        }
    "#;
    let err = type_check(input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Duplicate"), "Error message should mention 'Duplicate': {}", msg);
    assert!(msg.contains("Duplicated"), "Error message should include the form name: {}", msg);
}

#[test]
fn test_error_message_undefined_form() {
    let input = r#"
        Widget takes MissingForm {
            go = |x| { 1 }
        }
    "#;
    let err = type_check(input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Undefined") || msg.contains("undefined"),
        "Error message should mention undefined form: {}", msg);
    assert!(msg.contains("MissingForm"),
        "Error message should include the form name: {}", msg);
}

#[test]
fn test_error_message_missing_method() {
    let input = r#"
        form Worker {
            work: (x: Int32) -> Int32
            rest: (x: Int32) -> Int32
        }
        Bot takes Worker {
            work = |x| { 42 }
        }
    "#;
    let err = type_check(input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("rest"), "Error message should name the missing method: {}", msg);
    assert!(msg.contains("Worker"), "Error message should name the form: {}", msg);
}

#[test]
fn test_error_message_missing_associated_type() {
    let input = r#"
        form Transformer<T> {
            type Output
            transform: (x: T) -> Int32
        }
        MyTrans<T> takes Transformer<T> {
            transform = |x| { 0 }
        }
    "#;
    let err = type_check(input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Output"), "Error message should name the missing associated type: {}", msg);
    assert!(msg.contains("Transformer"), "Error message should name the form: {}", msg);
}

// ========================================================================
// 4. Code generation tests for form/takes monomorphization
// ========================================================================

fn codegen_ok(input: &str) -> String {
    let program = parse_ok(input);
    let mut codegen = WasmCodeGen::new();
    match codegen.generate(&program) {
        Ok(wat) => wat,
        Err(e) => panic!("Codegen failed: {:?}", e),
    }
}

#[test]
fn test_codegen_form_and_takes_stores_form_info() {
    // Verify that form definitions and takes declarations are processed
    // without error during code generation.
    let input = r#"
        form Showable {
            show: (self) -> String
        }
        MyType takes Showable {
            show = |self| { "hello" }
        }
        fun main() {
            0
        }
    "#;
    let wat = codegen_ok(input);
    // The WAT should contain the takes method implementation
    assert!(wat.contains("MyType_Showable_show"),
        "WAT should contain mangled takes method name: {}", wat);
}

#[test]
fn test_codegen_takes_generates_method_function() {
    // A takes declaration should generate a WASM function for each method impl
    let input = r#"
        form Computable {
            compute: (x: Int32) -> Int32
        }
        Num takes Computable {
            compute = |x| { 42 }
        }
        fun main() {
            0
        }
    "#;
    let wat = codegen_ok(input);
    // Should contain a function definition for the takes method
    assert!(wat.contains("(func $Num_Computable_compute"),
        "WAT should contain takes method function definition");
}

#[test]
fn test_codegen_multiple_takes_methods() {
    // Multiple methods in a takes declaration should each generate a function
    let input = r#"
        form MathOps {
            add: (a: Int32, b: Int32) -> Int32
            multiply: (a: Int32, b: Int32) -> Int32
        }
        Calculator takes MathOps {
            add = |a, b| { 0 }
            multiply = |a, b| { 1 }
        }
        fun main() {
            0
        }
    "#;
    let wat = codegen_ok(input);
    assert!(wat.contains("Calculator_MathOps_add"),
        "WAT should contain add method");
    assert!(wat.contains("Calculator_MathOps_multiply"),
        "WAT should contain multiply method");
}
