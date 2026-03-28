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
            fmap: (self, f: T) -> U
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
            size: (self) -> Int32
            empty: () -> Self
            append: (self, elem: T) -> Self
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
            fold<U>: (self, init: U) -> U
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
            length: (self) -> Int32
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
            show: (self) -> String
        }

        MyType takes Showable {
            show = |self| { "hello" }
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
            append: (self, elem: T) -> Self
        }

        MyList<T> takes Container<T> {
            append = |self, elem| { self }
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
            fmap: (self, f: T) -> U
        }

        MyList<T> takes Functor<T> {
            type Mapped<U> = MyList<U>
            fmap = |self, f| { self }
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
            show: (self) -> String
        }

        fun display<T of Showable>(x: T) -> String {
            (x) show
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
            show: (self) -> String
        }
        form Comparable {
            compare: (self, other: Self) -> Int32
        }

        fun process<T of Showable + Comparable>(x: T) -> String {
            (x) show
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
    let input = r#"
        form Showable {
            show: (self) -> String
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_form_with_associated_types_registers() {
    let input = r#"
        form Container<T> {
            type Mapped<U>
            fold<U>: (self, init: U) -> U
            append: (self, elem: T) -> Self
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_duplicate_form_gives_error() {
    let input = r#"
        form Showable {
            show: (self) -> String
        }
        form Showable {
            display: (self) -> String
        }
    "#;
    let result = type_check(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        TypeError::DuplicateForm(name) => {
            assert_eq!(name, "Showable");
        }
        other => panic!("Expected DuplicateForm error, got {:?}", other),
    }
}

#[test]
fn test_typecheck_takes_undefined_form_gives_error() {
    let input = r#"
        MyType takes NonExistentForm {
            foo = |self| { 42 }
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
        form Showable {
            show: (self) -> String
            describe: (self) -> String
        }

        MyType takes Showable {
            show = |self| { "hello" }
        }
    "#;
    let result = type_check(input);
    assert!(result.is_err());
    match result.unwrap_err() {
        TypeError::MissingFormMethod { form, method } => {
            assert_eq!(form, "Showable");
            assert_eq!(method, "describe");
        }
        other => panic!("Expected MissingFormMethod error, got {:?}", other),
    }
}

#[test]
fn test_typecheck_takes_missing_associated_type_gives_error() {
    let input = r#"
        form Functor<T> {
            type Mapped<U>
            fmap: (self, f: T) -> U
        }

        MyList<T> takes Functor<T> {
            fmap = |self, f| { self }
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
        form Showable {
            show: (self) -> String
        }

        MyType takes Showable {
            show = |self| { "hello" }
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_valid_takes_with_associated_type_succeeds() {
    let input = r#"
        form Functor<T> {
            type Mapped<U>
            fmap: (self, f: T) -> U
        }

        MyList<T> takes Functor<T> {
            type Mapped<U> = MyList<U>
            fmap = |self, f| { self }
        }
    "#;
    assert!(type_check(input).is_ok());
}

#[test]
fn test_typecheck_multiple_takes_for_same_type() {
    let input = r#"
        form Showable {
            show: (self) -> String
        }
        form Countable {
            count: (self) -> Int32
        }

        MyType takes Showable {
            show = |self| { "hello" }
        }
        MyType takes Countable {
            count = |self| { 0 }
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

// ========================================================================
// 3. Error message tests
// ========================================================================

#[test]
fn test_error_message_duplicate_form() {
    let input = r#"
        form Duplicated {
            run: (self) -> Int32
        }
        form Duplicated {
            execute: (self) -> Int32
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
            go = |self| { 1 }
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
            work: (self) -> Int32
            rest: (self) -> Int32
        }
        Bot takes Worker {
            work = |self| { 42 }
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
            transform: (self, x: T) -> Int32
        }
        MyTrans<T> takes Transformer<T> {
            transform = |self, x| { 0 }
        }
    "#;
    let err = type_check(input).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("Output"), "Error message should name the missing associated type: {}", msg);
    assert!(msg.contains("Transformer"), "Error message should name the form: {}", msg);
}
