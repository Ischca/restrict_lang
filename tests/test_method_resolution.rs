use restrict_lang::{parse_program, TypeChecker};

fn type_check(input: &str) -> Result<(), String> {
    let (remaining, program) = parse_program(input).map_err(|e| format!("Parse error: {:?}", e))?;
    if !remaining.trim().is_empty() {
        return Err(format!("Unparsed input remaining: {:?}", remaining));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error: {}", e))
}

#[test]
fn osv_function_call_replaces_method_call() {
    let input = r#"
record Point {
    x: Int32,
    y: Int32
}

fun manhattan: (point: Point) -> Int32 = {
    point.x + point.y
}

fun main: () -> Int32 = {
    val point = Point { x: 3, y: 4 };
    point |> manhattan
}
"#;

    type_check(input).expect("OSV function call should be the method-like form");
}

#[test]
fn field_access_and_osv_function_composition_coexist() {
    let input = r#"
record Data {
    value: Int32
}

fun double_value: (data: Data) -> Int32 = {
    data.value * 2
}

fun main: () -> Int32 = {
    val data = Data { value: 21 };
    val field_access = data.value;
    val transformed = data |> double_value;
    field_access + transformed
}
"#;

    type_check(input).expect("field access and OSV calls should type check");
}

#[test]
fn impl_method_resolves_from_receiver_type_without_double_consuming_receiver() {
    let input = r#"
record Score {
    value: Int32
}

impl Score {
    fun total: (self: Score) -> Int32 = {
        self.value
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 42 };
    (score) total
}
"#;

    type_check(input).expect("method dispatch should consume the affine receiver exactly once");
}

#[test]
fn impl_method_pipe_dispatch_is_not_current_release_surface() {
    let input = r#"
record Score {
    value: Int32
}

impl Score {
    fun total: (self: Score) -> Int32 = {
        self.value
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 42 };
    score |> total
}
"#;

    let err = type_check(input).expect_err("impl methods should use grouped OSV dispatch");
    assert!(
        err.contains("Type error") && err.contains("Score"),
        "error should reject pipe dispatch for impl methods in v0.0.1, got: {}",
        err
    );
}

#[test]
fn impl_method_receiver_must_be_self_with_target_record_type() {
    let input = r#"
record Score {
    value: Int32
}

impl Score {
    fun total: (score: Score) -> Int32 = {
        score.value
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 42 };
    (score) total
}
"#;

    let err = type_check(input).expect_err("impl receiver should be self: Target");
    assert!(
        err.contains("must declare first parameter as self: Score"),
        "error should explain the impl receiver contract, got: {}",
        err
    );
}

#[test]
fn impl_method_dispatch_on_record_returning_expression() {
    let input = r#"
record Score {
    value: Int32
}

impl Score {
    fun total: (self: Score) -> Int32 = {
        self.value
    }
}

fun make_score: () -> Score = {
    Score { value: 41 }
}

fun main: () -> Int32 = {
    (() make_score) total
}
"#;

    type_check(input).expect("method dispatch should resolve record-returning expressions");
}

#[test]
fn function_typed_record_field_can_be_called_as_pipe_target() {
    let input = r#"
record Strategy {
    mapper: Int32 -> Int32
}

fun main: () -> Int32 = {
    val strategy = Strategy {
        mapper: |score| score + 1
    };
    41 |> (strategy.mapper)
}
"#;

    type_check(input).expect("function-typed record fields should be callable pipe targets");
}

#[test]
fn generic_impl_method_instantiates_from_osv_arguments() {
    let input = r#"
record Box {
    value: Int32
}

impl Box {
    fun keep: <T>(self: Box, value: T) -> T = {
        value
    }
}

fun main: () -> String = {
    val box = Box { value: 1 };
    (box, "ok") keep
}
"#;

    type_check(input).expect("generic method calls should infer type params from OSV arguments");
}

#[test]
fn impl_method_calls_are_declaration_order_independent() {
    let input = r#"
fun read_score: (score: Score) -> Int32 = {
    (score) total
}

impl Score {
    fun total: (self: Score) -> Int32 = {
        self.value
    }
}

record Score {
    value: Int32
}

fun main: () -> Int32 = {
    val score = Score { value: 42 };
    score |> read_score
}
"#;

    type_check(input).expect("method signatures should be available before body checking");
}

#[test]
fn unannotated_impl_method_return_is_inferred_before_function_bodies() {
    let input = r#"
fun read_risk: (score: Score) -> Float64 = {
    (score) risk
}

impl Score {
    fun risk: (self: Score) = {
        self.value + 0.5
    }
}

record Score {
    value: Float64
}

fun main: () -> Float64 = {
    val score = Score { value: 41.5 };
    score |> read_risk
}
"#;

    type_check(input).expect("unannotated method return should be inferred before function bodies");
}

#[test]
fn annotated_impl_method_can_call_later_annotated_method() {
    let input = r#"
record Score {
    value: Float64
}

impl Score {
    fun adjusted: (self: Score) -> Float64 = {
        (self) risk
    }

    fun risk: (self: Score) -> Float64 = {
        self.value + 0.5
    }
}

fun main: () -> Float64 = {
    val score = Score { value: 41.5 };
    (score) adjusted
}
"#;

    type_check(input).expect("annotated methods should support method-to-method forward calls");
}

#[test]
fn unannotated_impl_method_forward_call_requires_return_annotation() {
    let input = r#"
record Score {
    value: Float64
}

impl Score {
    fun adjusted: (self: Score) = {
        (self) risk
    }

    fun risk: (self: Score) = {
        self.value + 0.5
    }
}

fun main: () -> Float64 = {
    val score = Score { value: 41.5 };
    (score) adjusted
}
"#;

    let err = type_check(input).expect_err("unannotated forward method calls should be explicit");
    assert!(
        err.contains(
            "method 'risk' for record 'Score' is used before its return type has been inferred"
        ),
        "error should explain the required return annotation, got: {}",
        err
    );
}

#[test]
fn same_method_name_resolves_by_receiver_record() {
    let input = r#"
record Score {
    value: Int32
}

record Penalty {
    value: Int32
}

impl Score {
    fun amount: (self: Score) -> Int32 = {
        self.value
    }
}

impl Penalty {
    fun amount: (self: Penalty) -> Int32 = {
        0 - self.value
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 7 };
    (score) amount
}
"#;

    type_check(input).expect("receiver type should disambiguate shared method names");
}

#[test]
fn duplicate_methods_on_same_record_are_rejected() {
    let input = r#"
record Score {
    value: Int32
}

impl Score {
    fun amount: (self: Score) -> Int32 = {
        self.value
    }
}

impl Score {
    fun amount: (self: Score) -> Int32 = {
        0
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 7 };
    (score) amount
}
"#;

    let err = type_check(input).expect_err("duplicate methods should be rejected");
    assert!(
        err.contains("Duplicate method 'amount' for record 'Score'"),
        "error should explain the duplicate method, got: {}",
        err
    );
}

#[test]
fn traditional_dot_method_call_is_rejected() {
    let input = r#"
record Data {
    value: Int32
}

fun main: () -> Int32 = {
    val data = Data { value: 42 };
    data.getValue()
}
"#;

    let err = type_check(input).expect_err("traditional method syntax should be rejected");
    assert!(
        err.contains("Parse error") || err.contains("Unparsed input"),
        "error should come from syntax rejection, got: {}",
        err
    );
}

#[test]
fn traditional_dot_impl_method_call_is_rejected() {
    let input = r#"
record Score {
    value: Int32
}

impl Score {
    fun amount: (self: Score) -> Int32 = {
        self.value
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 42 };
    score.amount()
}
"#;

    let err = type_check(input).expect_err("traditional impl method syntax should be rejected");
    assert!(
        err.contains("Parse error") || err.contains("Unparsed input"),
        "error should come from syntax rejection, got: {}",
        err
    );
}

#[test]
fn wrong_osv_argument_type_is_rejected() {
    let input = r#"
record Calculator {
    value: Int32
}

fun add_value: (calc: Calculator, amount: Int32) -> Int32 = {
    calc.value + amount
}

fun main: () -> Int32 = {
    val calc = Calculator { value: 10 };
    (calc, "not a number") add_value
}
"#;

    let err = type_check(input).expect_err("wrong OSV argument type should fail");
    assert!(
        err.contains("Type error"),
        "error should explain type failure, got: {}",
        err
    );
}

#[test]
fn wrong_osv_arity_is_rejected() {
    let input = r#"
record Adder {
    base: Int32
}

fun add: (adder: Adder, a: Int32, b: Int32) -> Int32 = {
    adder.base + a + b
}

fun main: () -> Int32 = {
    val adder = Adder { base: 1 };
    (adder, 2) add
}
"#;

    let err = type_check(input).expect_err("wrong OSV arity should fail");
    assert!(
        err.contains("Type error"),
        "error should explain type failure, got: {}",
        err
    );
}
