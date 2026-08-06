use restrict_lang::parser::parse_program;
use restrict_lang::type_checker::TypeChecker;
use restrict_lang::WasmCodeGen;

fn check(source: &str) -> Result<(), String> {
    let (remaining, program) = parse_program(source).map_err(|error| format!("{error:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("unparsed input: {remaining:?}"));
    }
    TypeChecker::new()
        .check_program(&program)
        .map_err(|error| error.to_string())
}

fn compile(source: &str) -> Result<String, String> {
    let (remaining, program) = parse_program(source).map_err(|error| format!("{error:?}"))?;
    if !remaining.trim().is_empty() {
        return Err(format!("unparsed input: {remaining:?}"));
    }
    TypeChecker::new()
        .check_program(&program)
        .map_err(|error| error.to_string())?;
    WasmCodeGen::new()
        .generate(&program)
        .map_err(|error| error.to_string())
}

#[test]
fn custom_form_adoption_and_generic_dispatch_type_check() {
    check(
        r#"
form Showable {
    fun show: (self: Self) -> String
}

record Widget {
    value: Int32
}

Widget takes Showable {
    fun show: (self: Widget) -> String = {
        "widget"
    }
}

fun render: <T of Showable>(value: T) -> String = {
    value |> show
}

fun main: () -> String = {
    Widget { value: 7 } |> render
}
"#,
    )
    .expect("custom form programs should type-check");
}

#[test]
fn form_method_receivers_support_complex_expressions_without_affine_replay() {
    let source = r#"
form Showable {
    fun show: (self: Self) -> String
}

record Item {
    value: Int32
}

record Holder {
    inner: Item
}

Item takes Showable {
    fun show: (self: Item) -> String = { "item" }
}

fun show_field: (holder: Holder) -> String = {
    holder.inner |> show
}

fun show_conditional: (choose_left: Boolean, left: Item, right: Item) -> String = {
    (choose_left then { left } else { right }) |> show
}

fun show_match: (choose_left: Boolean, left: Item, right: Item) -> String = {
    (choose_left match {
        true => { left }
        false => { right }
    }) |> show
}

fun show_block: (item: Item) -> String = {
    ({
        val selected = item
        selected
    }) |> show
}
"#;

    let wat = compile(source).expect("complex form receivers should type-check and lower to Wasm");
    let wasm = wat::parse_str(&wat).expect("complex form receivers should generate valid WAT");
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .expect("complex form receivers should generate a valid Wasm module");
}

#[test]
fn complex_receiver_probe_does_not_claim_a_new_pipe_binding() {
    check(
        r#"
record Item { value: Int32 }

fun select: (choose_left: Boolean, left: Item, right: Item) -> Item = {
    (choose_left then { left } else { right }) |> selected
    selected
}
"#,
    )
    .expect("a complex receiver with no matching selector should remain a pipe binding");
}

#[test]
fn form_adoptions_must_implement_the_exact_contract() {
    let missing = check(
        r#"
form Showable {
    fun show: (self: Self) -> String
}
record Widget { value: Int32 }
Widget takes Showable {}
"#,
    )
    .expect_err("a required form method may not be omitted");
    assert!(missing.contains("Missing method show required by form Showable"));

    let mismatched = check(
        r#"
form Showable {
    fun show: (self: Self) -> String
}
record Widget { value: Int32 }
Widget takes Showable {
    fun show: (self: Widget) -> Int32 = { 1 }
}
"#,
    )
    .expect_err("an adoption signature must match its form");
    assert!(mismatched.contains("form method signature mismatch"));
}

#[test]
fn form_bounds_require_a_declared_form_and_matching_adoption() {
    let unknown = check(
        r#"
fun render: <T of Missing>(value: T) -> T = { value }
"#,
    )
    .expect_err("unknown form bounds should fail");
    assert!(unknown.contains("Form Missing is not defined"));

    let missing_adoption = check(
        r#"
form Showable {
    fun show: (self: Self) -> String
}
record Widget { value: Int32 }
fun render: <T of Showable>(value: T) -> String = { value |> show }
fun main: () -> String = { Widget { value: 7 } |> render }
"#,
    )
    .expect_err("a constrained call needs a matching adoption");
    assert!(
        missing_adoption.contains("Showable"),
        "diagnostic should identify the unsatisfied form: {missing_adoption}"
    );
}

#[test]
fn duplicate_and_ambiguous_form_adoptions_are_rejected() {
    let duplicate = check(
        r#"
form Showable { fun show: (self: Self) -> String }
record Widget { value: Int32 }
Widget takes Showable {
    fun show: (self: Widget) -> String = { "one" }
}
Widget takes Showable {
    fun show: (self: Widget) -> String = { "two" }
}
"#,
    )
    .expect_err("a type may adopt a form only once");
    assert!(duplicate.contains("already takes form Showable"));

    let ambiguous = check(
        r#"
form First { fun label: (self: Self) -> String }
form Second { fun label: (self: Self) -> String }
fun render: <T of First + Second>(value: T) -> String = { value |> label }
"#,
    )
    .expect_err("shared selectors across bounds need an unambiguous form");
    assert!(ambiguous.contains("ambiguous"));
}

#[test]
fn display_supports_scalars_and_explicit_record_adoptions() {
    check(
        r#"
record Notice { code: Int32 }
Notice takes Display {
    fun display: (self: Notice) -> String = { "notice" }
}
fun render: <T of Display>(value: T) -> String = {
    value |> display
}
fun write: <T of Display>(value: T) -> () = {
    value |> println
}
fun main: () -> () = {
    42 |> print;
    true |> println;
    Notice { code: 7 } |> println
}
"#,
    )
    .expect("Display-backed print functions should accept scalars and adopted records");
}

#[test]
fn display_is_reserved_and_records_do_not_adopt_it_implicitly() {
    let reserved = check(
        r#"
form Display { fun display: (self: Self) -> String }
"#,
    )
    .expect_err("the standard Display form may not be redeclared");
    assert!(reserved.contains("Form Display is already defined"));

    let implicit = check(
        r#"
record Notice { code: Int32 }
fun main: () -> () = { Notice { code: 7 } |> println }
"#,
    )
    .expect_err("records must explicitly take Display");
    assert!(
        implicit.contains("Display"),
        "diagnostic should identify the missing Display adoption: {implicit}"
    );
}

#[test]
fn bounded_records_validate_declarations_annotations_and_construction() {
    let unknown = check(
        r#"
record Envelope<T of Missing> { value: T }
"#,
    )
    .expect_err("record bounds must name a declared form");
    assert!(unknown.contains("Form Missing is not defined"));

    let invalid_annotation = check(
        r#"
record Plain { value: Int32 }
record Envelope<T of Display> { value: T }
fun reject: (value: Envelope<Plain>) -> () = { () }
"#,
    )
    .expect_err("bounded record annotations must satisfy their forms");
    assert!(invalid_annotation.contains("Display"));

    let invalid_literal = check(
        r#"
record Plain { value: Int32 }
record Envelope<T of Display> { value: T }
fun main: () -> () = {
    val envelope = Envelope { value: Plain { value: 1 } };
    ()
}
"#,
    )
    .expect_err("bounded record construction must satisfy inferred form obligations");
    assert!(invalid_literal.contains("Display"));

    check(
        r#"
record Envelope<T of Display> { value: T }
fun wrap: <T of Display>(value: T) -> Envelope<T> = {
    Envelope { value: value }
}
fun main: () -> Envelope<Int32> = { 42 |> wrap }
"#,
    )
    .expect("concrete and abstract evidence should satisfy bounded record uses");
}

#[test]
fn generic_function_value_preserves_form_obligations() {
    let missing = check(
        r#"
record Plain { value: Int32 }
fun render: <T of Display>(value: T) -> String = { value |> display }
fun main: () -> String = {
    val f = render;
    Plain { value: 1 } |> f
}
"#,
    )
    .expect_err("a deferred generic function value must retain its Display obligation");
    assert!(missing.contains("Display"));

    check(
        r#"
record Label { value: Int32 }
Label takes Display {
    fun display: (self: Label) -> String = { "label" }
}
fun render: <T of Display>(value: T) -> String = { value |> display }
fun main: () -> String = {
    val f = render;
    Label { value: 1 } |> f
}
"#,
    )
    .expect("a deferred generic function value should accept a matching adoption");
}

#[test]
fn source_cannot_name_internal_container_in_of_bounds() {
    for source in [
        "fun inspect: <T of Container>(value: T) -> T = { value }",
        "record InternalBox<T of Container> { value: T }",
        "fun inspect: <T: Container>(value: T) -> T = { value }",
    ] {
        let error = check(source).expect_err("Container is not a source-visible bound");
        assert!(
            error.contains("compiler-internal") && error.contains("Container"),
            "diagnostic should explain the internal boundary: {error}"
        );
    }

    let adoption = check(
        r#"
record Item { value: Int32 }
Item takes Container {}
"#,
    )
    .expect_err("Container is not a source-visible adoption target");
    assert!(
        adoption.contains("compiler-internal") && adoption.contains("Container"),
        "diagnostic should explain the internal adoption boundary: {adoption}"
    );
}

#[test]
fn overlapping_form_selectors_are_rejected_at_the_bound_declaration() {
    let error = check(
        r#"
form First { fun label: (self: Self) -> String }
form Second { fun label: (self: Self) -> String }
fun unreachable: <T of First + Second>(value: T) -> T = { value }
"#,
    )
    .expect_err("an overlapping multi-form bound is uninhabitable in the initial slice");
    assert!(error.contains("ambiguous") && error.contains("label"));
}

#[test]
fn generic_impl_methods_validate_declared_form_bounds() {
    let missing = check(
        r#"
record Box { value: Int32 }
impl Box {
    fun keep: <T of Missing>(self: Box, value: T) -> T = { value }
}
"#,
    )
    .expect_err("generic impl methods must reject undefined form bounds");
    assert!(missing.contains("Missing"));

    let internal = check(
        r#"
record Box { value: Int32 }
impl Box {
    fun keep: <T of Container>(self: Box, value: T) -> T = { value }
}
"#,
    )
    .expect_err("generic impl methods must reject compiler-internal form bounds");
    assert!(internal.contains("compiler-internal") && internal.contains("Container"));

    let overlap = check(
        r#"
form First { fun label: (self: Self) -> String }
form Second { fun label: (self: Self) -> String }
record Box { value: Int32 }
impl Box {
    fun keep: <T of First + Second>(self: Box, value: T) -> T = { value }
}
"#,
    )
    .expect_err("generic impl methods must reject overlapping form selectors");
    assert!(overlap.contains("ambiguous") && overlap.contains("label"));
}

#[test]
fn applicable_form_selectors_precede_same_named_global_functions() {
    check(
        r#"
form Mappable { fun map: (self: Self) -> Int32 }
record Reading { value: Int32 }
Reading takes Mappable {
    fun map: (self: Reading) -> Int32 = { self.value }
}
fun piped: <T of Mappable>(value: T) -> Int32 = { value |> map }
fun called: <T of Mappable>(value: T) -> Int32 = { (value) map }
"#,
    )
    .expect("type-directed form dispatch should win over the prelude map function");
}

#[test]
fn form_selector_collisions_do_not_create_false_return_inference_cycles() {
    check(
        r#"
form Scored { fun score: (self: Self) -> Int32 }
record Reading { value: Int32 }
Reading takes Scored {
    fun score: (self: Reading) -> Int32 = { self.value }
}
fun score: () = { () helper }
fun helper: () = {
    val reading = Reading { value: 42 };
    reading |> score
}
fun main: () -> Int32 = { () score }
"#,
    )
    .expect("method-selected calls must not depend on a same-named unannotated global");
}

#[test]
fn scoped_method_receivers_do_not_create_false_inference_cycles() {
    check(
        r#"
form ScoredPaths {
    fun match_score: (self: Self) -> Int32
    fun lambda_score: (self: Self) -> Int32
    fun with_score: (self: Self) -> Int32
    fun call_score: (self: Self) -> Int32
    fun pipe_score: (self: Self) -> Int32
}
record Reading { value: Int32 }
Reading takes ScoredPaths {
    fun match_score: (self: Reading) -> Int32 = { self.value }
    fun lambda_score: (self: Reading) -> Int32 = { self.value }
    fun with_score: (self: Reading) -> Int32 = { self.value }
    fun call_score: (self: Reading) -> Int32 = { self.value }
    fun pipe_score: (self: Reading) -> Int32 = { self.value }
}
context ScoreContext { reading: Reading }

fun match_score: () = { () match_helper }
fun match_helper: () = {
    (Reading { value: 40 }) Option::Some match {
        Some(reading) => { reading |> match_score }
        None => { 0 }
    }
}

fun lambda_score: () = { () lambda_helper }
fun lambda_helper: () = {
    val scorer: Reading -> Int32 = |reading| reading |> lambda_score;
    Reading { value: 41 } |> scorer
}

fun with_score: () = { () with_helper }
fun with_helper: () = {
    with ScoreContext { reading: Reading { value: 42 } } {
        reading |> with_score
    }
}

fun apply_score: (scorer: Reading -> Int32) -> Int32 = {
    Reading { value: 43 } |> scorer
}
fun call_score: () = { () call_helper }
fun call_helper: () = {
    (|reading| reading |> call_score) apply_score
}

fun pipe_score: () = { () pipe_helper }
fun pipe_helper: () = {
    (|reading| reading |> pipe_score) |> apply_score
}
"#,
    )
    .expect("scoped typed receivers must select methods without false global dependencies");
}

#[test]
fn form_contract_types_must_satisfy_bounded_record_requirements() {
    let error = check(
        r#"
record Plain { value: Int32 }
record Envelope<T of Display> { value: T }
form Invalid {
    fun wrap: (self: Self) -> Envelope<Plain>
}
"#,
    )
    .expect_err("form signatures cannot hide an invalid bounded record use");
    assert!(
        error.contains("Display"),
        "diagnostic should identify the unsatisfied record bound: {error}"
    );
}

#[test]
fn form_named_copy_does_not_grant_affine_copy_semantics() {
    let error = check(
        r#"
form Copy {
    fun inspect: (self: Self) -> String
}
fun duplicate: <T of Copy>(value: T) -> T = {
    val moved = value;
    value
}
"#,
    )
    .expect_err("form evidence named Copy must remain separate from affine Copy");
    assert!(error.contains("affine type violation"));

    let legacy_trait = check(
        r#"
form Copy {
    fun inspect: (self: Self) -> String
}
record Resource { name: String }
Resource takes Copy {
    fun inspect: (self: Resource) -> String = { "resource" }
}
fun require_copy: <T: Copy>(value: T) -> T = { value }
fun main: () -> Resource = {
    Resource { name: "owned" } |> require_copy
}
"#,
    )
    .expect_err("a source form adoption must not satisfy the legacy Copy trait");
    assert!(
        legacy_trait.contains("does not implement trait Copy"),
        "trait and form evidence must remain distinct: {legacy_trait}"
    );
}

#[test]
fn contexts_cannot_adopt_forms() {
    let error = check(
        r#"
form Labelled { fun label: (self: Self) -> String }
context Settings { value: Int32 }
Settings takes Labelled {
    fun label: (self: Settings) -> String = { "settings" }
}
"#,
    )
    .expect_err("a context must not be accepted as a concrete record takes target");
    assert!(error.contains("contexts cannot adopt forms"));
}

#[test]
fn display_intrinsic_names_have_an_explicit_source_boundary() {
    let top_level = check(
        r#"
fun print: (value: Int32) -> () = { () }
"#,
    )
    .expect_err("source functions cannot shadow a Display output intrinsic");
    assert!(top_level.contains("compiler-reserved"));

    let form_selector = check(
        r#"
form Custom { fun display: (self: Self) -> String }
"#,
    )
    .expect_err("custom forms cannot claim the intrinsic display selector");
    assert!(form_selector.contains("compiler-reserved"));

    let impl_selector = check(
        r#"
record Counter { value: Int32 }
impl Counter {
    fun println: (self: Counter) -> () = { () }
}
"#,
    )
    .expect_err("ordinary impls cannot claim output intrinsic selectors");
    assert!(impl_selector.contains("compiler-reserved"));

    let first_class = check(
        r#"
fun main: () -> String = {
    val formatter: Int32 -> String = display;
    1 |> formatter
}
"#,
    )
    .expect_err("Display output intrinsics are not first-class in the initial slice");
    assert!(first_class.contains("first-class function value"));

    check(
        r#"
fun main: () -> Int32 = {
    val print: Int32 -> Int32 = |value: Int32| value;
    1 |> print
}
"#,
    )
    .expect("local callable bindings remain ordinary lexical bindings");
}

#[test]
fn form_contract_parameter_names_are_not_part_of_the_signature() {
    check(
        r#"
form Combine {
    fun combine: (self: Self, left: Int32) -> String
}
record Widget { value: Int32 }
Widget takes Combine {
    fun combine: (self: Widget, right: Int32) -> String = { "combined" }
}
fun main: () -> String = { (Widget { value: 1 }, 2) combine }
"#,
    )
    .expect("form adoption parameter names may differ when positional types match");
}

#[test]
fn form_contracts_and_adoptions_reject_duplicate_parameter_binders() {
    let contract = check(
        r#"
form Broken {
    fun combine: (self: Self, value: Int32, value: Int32) -> String
}
"#,
    )
    .expect_err("a form contract cannot bind one parameter name twice");
    assert!(contract.contains("duplicate parameter 'value'"));

    let adoption = check(
        r#"
form Combine {
    fun combine: (self: Self, left: Int32, right: Int32) -> String
}
record Widget { value: Int32 }
Widget takes Combine {
    fun combine: (self: Widget, value: Int32, value: Int32) -> String = { "bad" }
}
"#,
    )
    .expect_err("a form adoption cannot bind one parameter name twice");
    assert!(adoption.contains("duplicate parameter 'value'"));
}
