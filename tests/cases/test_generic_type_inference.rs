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
fn generic_identity_infers_from_argument() {
    let input = r#"
fun identity_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    42 |> identity_local
}
"#;

    type_check(input).expect("generic identity should infer T from the piped argument");
}

#[test]
fn generic_function_infers_multiple_parameters() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> String = {
    ("primary", "fallback") choose_first
}
"#;

    type_check(input).expect("generic call should infer a shared type parameter");
}

#[test]
fn generic_function_infers_annotated_lambda_from_bare_type_parameter() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun apply_once: (f: Int32 -> Int32) -> Int32 = {
    41 |> f
}

fun main: () -> Int32 = {
    val chosen = (|x: Int32| x + 1, |y: Int32| y) choose_first
    chosen |> apply_once
}
"#;

    type_check(input)
        .expect("annotated lambdas should infer a shared function type through generic arguments");
}

#[test]
fn generic_function_shapes_unannotated_lambda_from_bare_type_parameter() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val chosen = (|x| x + 1, |y| y) choose_first
    41 |> chosen
}
"#;

    type_check(input).expect("bare generic T should shape unannotated lambdas as function values");
}

#[test]
fn generic_function_shapes_unannotated_lambda_inside_option() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val chosen = ((|x| x + 1) Option::Some, (|y| y) Option::Some) choose_first
    chosen match {
        Some(mapper) => { 41 |> mapper }
        None => { 0 }
    }
}
"#;

    type_check(input).expect("bare generic Option<T> should shape lambda payloads");
}

#[test]
fn generic_function_shapes_unannotated_lambda_inside_list() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val chosen = ([|x| x + 1], [|y| y]) choose_first
    val mapper = (chosen, 0) list_get
    41 |> mapper
}
"#;

    type_check(input).expect("bare generic List<T> should shape lambda elements");
}

#[test]
fn prelude_map_infers_lambda_return_type() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n * 2) map
}
"#;

    type_check(input).expect("map should infer U from the lambda body");
}

#[test]
fn prelude_map_infers_implicit_focus_scope() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3];
    numbers map {
        it * 2
    }
}
"#;

    type_check(input).expect("map should infer the implicit focus type from its container");
}

#[test]
fn prelude_map_infers_explicit_scoped_binder() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3];
    numbers map { |number|
        val shifted = number + 1;
        shifted * 2
    }
}
"#;

    type_check(input).expect("map should type an explicit binder with an ordinary block body");
}

#[test]
fn user_function_can_open_its_final_function_parameter_as_a_scope() {
    let input = r#"
fun apply: (value: Int32, transform: Int32 -> Int32) -> Int32 = {
    value |> transform
}

fun main: () -> Int32 = {
    41 apply {
        it + 1
    }
}
"#;

    type_check(input).expect("scoped verb clauses should work for ordinary user functions");
}

#[test]
fn zero_parameter_scope_uses_an_explicit_empty_header() {
    let input = r#"
fun choose: (fallback: Int32, build: () -> Int32) -> Int32 = {
    () build
}

fun main: () -> Int32 = {
    0 choose { ||
        42
    }
}
"#;

    type_check(input).expect("an explicit empty header should supply a zero-parameter scope");
}

#[test]
fn scoped_collection_clauses_chain_left_associatively() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3];
    numbers map {
        it + 1
    } filter {
        it > 2
    }
}
"#;

    type_check(input).expect("each completed scoped clause should feed the next verb");
}

#[test]
fn nested_implicit_focus_requires_an_explicit_binder() {
    let input = r#"
fun main: () -> List<List<Int32>> = {
    val groups: List<List<Int32>> = [[1, 2], [3]];
    groups map {
        it map {
            it + 1
        }
    }
}
"#;

    let err = type_check(input).expect_err("nested implicit focus scopes should be rejected");
    assert!(
        err.contains("nested implicit focus scopes") && err.contains("|value|"),
        "diagnostic should require an explicit scope binder, got: {err}"
    );
}

#[test]
fn explicit_outer_binder_allows_nested_implicit_focus() {
    let input = r#"
fun main: () -> List<List<Int32>> = {
    val groups: List<List<Int32>> = [[1, 2], [3]];
    groups map { |group|
        group map {
            it + 1
        }
    }
}
"#;

    type_check(input).expect("an explicit outer binder should disambiguate nested focus scopes");
}

#[test]
fn prelude_map_uses_expected_return_type() {
    let input = r#"
fun main: () -> List<String> = {
    val names: List<String> = ["a", "b"]
    (names, |name| name) map
}
"#;

    type_check(input).expect("map should preserve the expected List element type");
}

#[test]
fn prelude_map_uses_container_mapped_projection_for_option() {
    let input = r#"
fun main: () -> Option<String> = {
    val maybe_score: Option<Int32> = (7) Option::Some
    (maybe_score, |score| score > 0 then { "positive" } else { "zero" }) map
}
"#;

    type_check(input).expect("map should infer Option<Int32>.Mapped<String> as Option<String>");
}

#[test]
fn prelude_map_accepts_monomorphic_named_function_mapper() {
    let input = r#"
fun score: (value: Int32) -> Float64 = {
    value > 0 then {
        1.5
    } else {
        0.5
    }
}

fun main: () -> List<Float64> = {
    val numbers = [1, 2, 3]
    (numbers, score) map
}
"#;

    type_check(input).expect("map should accept a named monomorphic function as mapper");
}

#[test]
fn prelude_identity_can_be_used_as_expected_function_value() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3]
    (numbers, identity) map
}
"#;

    type_check(input).expect("identity should instantiate from the mapper expected type");
}

#[test]
fn user_generic_function_value_instantiates_from_expected_mapper() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3]
    (numbers, id_local) map
}
"#;

    type_check(input).expect("user generic mapper should instantiate from expected function type");
}

#[test]
fn user_generic_function_value_still_requires_expected_function_type() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val mapper = id_local;
    0
}
"#;

    let err = type_check(input).expect_err("unresolved generic function value should be rejected");
    assert!(
        err.contains("Cannot infer type") && err.contains("mapper"),
        "error should identify the unresolved generic function value, got: {}",
        err
    );
}

#[test]
fn top_level_builtin_function_value_still_requires_expected_function_type() {
    let input = r#"
val apply_map = map
"#;

    let err =
        type_check(input).expect_err("top-level unresolved builtin function value should reject");
    assert!(
        err.contains("Cannot infer type") && err.contains("apply_map"),
        "error should identify the unresolved top-level function value, got: {}",
        err
    );
    for internal in ["?0", "InferVar", "TypeVarId", "Projection"] {
        assert!(
            !err.contains(internal),
            "error should not expose inference internals ({internal}), got: {err}"
        );
    }
}

#[test]
fn local_generic_function_value_infers_from_later_map_use() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    val mapper = id_local;
    (numbers, mapper) map
}
"#;

    type_check(input).expect("local generic function value should infer from later map use");
}

#[test]
fn local_generic_function_alias_chain_infers_from_later_pipe_use() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val keep = id_local;
    val keep_again = keep;
    41 |> keep_again
}
"#;

    type_check(input).expect("generic function alias chains should infer from later pipe use");
}

#[test]
fn resolved_generic_function_alias_chain_rejects_double_use() {
    let input = r#"
fun id_local: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    val keep = id_local;
    val keep_again = keep;
    val first = 41 |> keep_again;
    val second = 42 |> keep;
    first + second
}
"#;

    let err = type_check(input)
        .expect_err("resolved non-copy function aliases should preserve affine double-use checks");
    assert!(
        err.contains("affine type violation"),
        "error should explain the affine double use, got: {err}"
    );
}

#[test]
fn direct_generic_function_alias_infers_from_single_pipe_use() {
    let input = r#"
fun main: () -> Int32 = {
    val keep = identity;
    41 |> keep
}
"#;

    type_check(input).expect("one use should resolve and consume a direct generic alias");
}

#[test]
fn direct_generic_function_alias_rejects_pending_double_use() {
    let input = r#"
fun main: () -> Int32 = {
    val keep = identity;
    keep;
    41 |> keep
}
"#;

    let err = type_check(input)
        .expect_err("resolving a direct generic alias must account for all pending uses");
    assert!(
        err.contains("affine type violation") && err.contains("keep"),
        "error should identify the direct generic alias double use, got: {err}"
    );
}

#[test]
fn projection_bearing_generic_alias_chain_rejects_source_reuse() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    val apply_map = map;
    val map_again = apply_map;
    apply_map;
    (numbers, |value| value + 1) map_again
}
"#;

    let err = type_check(input)
        .expect_err("a moved projection-bearing generic alias must not remain reusable");
    assert!(
        err.contains("affine type violation") && err.contains("apply_map"),
        "error should identify the moved map alias, got: {err}"
    );
}

#[test]
fn branch_merge_rejects_pending_affine_double_use_after_concretization() {
    let input = r#"
fun main: (flag: Boolean) -> () = {
    val items = [];
    flag then {
        items;
        items;
        ()
    } else {
        val typed: List<String> = items;
        ()
    }
}
"#;

    let err = type_check(input)
        .expect_err("branch type resolution must not erase two pending affine uses");
    assert!(
        err.contains("affine type violation") && err.contains("items"),
        "error should identify the branch-local affine double use, got: {err}"
    );
}

#[test]
fn branch_merge_clears_resolved_generic_deferred_state() {
    let input = r#"
fun main: (flag: Boolean) -> () = {
    val keep = identity;
    flag then {
        val typed: Int32 -> Int32 = keep;
        ()
    } else {
        ()
    }
}
"#;

    type_check(input)
        .expect("a generic alias resolved in one branch should not retain stale deferred state");
}

#[test]
fn branch_merge_clears_rechecked_immutable_lambda_deferred_state() {
    let input = r#"
fun main: (flag: Boolean) -> () = {
    val mapper = |value| value;
    flag then {
        val typed: Int32 -> Int32 = mapper;
        ()
    } else {
        ()
    }
}
"#;

    type_check(input).expect(
        "an immutable lambda rechecked in one branch should not retain stale deferred state",
    );
}

#[test]
fn branch_merge_fixes_mutable_lambda_to_concrete_specialization() {
    let input = r#"
fun main: (flag: Boolean) -> Boolean = {
    mut val id = |value| value;
    flag then {
        val as_int: Int32 -> Int32 = id;
        ()
    } else {
        ()
    };
    val as_bool: Boolean -> Boolean = id;
    true |> as_bool
}
"#;

    let err = type_check(input)
        .expect_err("branch specialization must fix a mutable deferred lambda's concrete type");
    assert!(
        err.contains("Type mismatch"),
        "the incompatible post-merge specialization should be rejected, got: {err}"
    );
}

#[test]
fn branch_merge_resolves_projection_bearing_direct_map_alias() {
    let input = r#"
fun main: (flag: Boolean) -> () = {
    val apply_map = map;
    flag then {
        val numbers = [1, 2, 3];
        val mapped = (numbers, |value| value + 1) apply_map;
        ()
    } else {
        ()
    }
}
"#;

    type_check(input)
        .expect("one branch should be able to specialize a projection-bearing map alias");
}

#[test]
fn branch_merge_resolves_projection_bearing_map_alias_chain() {
    let input = r#"
fun main: (flag: Boolean) -> () = {
    val map_plan = map;
    val apply_map = map_plan;
    flag then {
        val numbers = [1, 2, 3];
        val mapped = (numbers, |value| value + 1) apply_map;
        ()
    } else {
        ()
    }
}
"#;

    type_check(input)
        .expect("one branch should specialize a complete projection-bearing alias chain");
}

#[test]
fn branch_merge_rejects_incompatible_generic_alias_specializations() {
    let input = r#"
fun main: (flag: Boolean) -> () = {
    val apply_map = map;
    flag then {
        val numbers = [1, 2, 3];
        val mapped = (numbers, |value| value + 1) apply_map;
        ()
    } else {
        val words = ["one", "two"];
        val mapped = (words, |value| value) apply_map;
        ()
    }
}
"#;

    let err = type_check(input)
        .expect_err("one generic alias cannot have incompatible branch specializations");
    assert!(
        err.contains("Type mismatch"),
        "error should report incompatible branch specializations, got: {err}"
    );
    assert!(
        !err.contains("unresolved deferred type"),
        "incompatible specializations should fail at the merge, got: {err}"
    );
}

#[test]
fn substitution_rejects_use_after_incompatible_affine_branch_consumption() {
    let input = r#"
fun main: (flag: Boolean) -> () = {
    val items = [];
    flag then {
        val strings: List<String> = items;
        ()
    } else {
        val integers: List<Int32> = items;
        ()
    };
    val reused: List<String> = items;
    ()
}
"#;

    let err = type_check(input)
        .expect_err("later inference must retain affine uses from incompatible branches");
    assert!(
        err.contains("affine type violation") && err.contains("items"),
        "error should identify the affine value consumed in a branch, got: {err}"
    );
}

#[test]
fn branch_merge_validates_generic_alias_concretized_by_generic_sink() {
    let input = r#"
form Marker {
    fun mark: (self: Self) -> String
}

fun bounded: <T of Marker>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun plain: (value: Int32) -> Int32 = {
    value
}

fun main: (flag: Boolean) -> () = {
    val bounded_alias = bounded;
    val picked = (bounded_alias, plain) choose_first;
    flag then {
        ()
    } else {
        ()
    }
}
"#;

    let err = type_check(input)
        .expect_err("branch merge must validate a concrete deferred generic against its bounds");
    assert!(
        err.contains("Marker"),
        "error should identify the unsatisfied stored generic bound, got: {err}"
    );
    assert!(
        !err.contains("unresolved deferred type"),
        "the concrete generic alias should fail validation before scope finalization, got: {err}"
    );
}

#[test]
fn generic_sink_concretized_alias_is_validated_without_branch_merge() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun plain: (value: Int32) -> Int32 = {
    value
}

fun main: () -> Int32 = {
    val identity_alias = identity;
    val picked = (identity_alias, plain) choose_first;
    41 |> picked
}
"#;

    type_check(input)
        .expect("scope finalization should validate a concrete generic alias without a branch");
}

#[test]
fn generic_sink_fixes_mutable_alias_to_concrete_specialization() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun plain: (value: Int32) -> Int32 = {
    value
}

fun main: () -> Boolean = {
    mut val id = identity;
    val picked = (id, plain) choose_first;
    val as_bool: Boolean -> Boolean = id;
    true |> as_bool
}
"#;

    let err = type_check(input)
        .expect_err("generic-sink inference must fix a mutable alias's concrete type");
    assert!(
        err.contains("Type mismatch"),
        "the incompatible later specialization should be rejected, got: {err}"
    );
}

#[test]
fn generic_sink_concretized_alias_rejects_unsatisfied_bound_without_branch() {
    let input = r#"
form Marker {
    fun mark: (self: Self) -> String
}

fun bounded: <T of Marker>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun plain: (value: Int32) -> Int32 = {
    value
}

fun main: () -> Int32 = {
    val bounded_alias = bounded;
    val picked = (bounded_alias, plain) choose_first;
    41 |> picked
}
"#;

    let err = type_check(input)
        .expect_err("scope finalization must enforce the stored generic alias bounds");
    assert!(
        err.contains("Marker"),
        "error should identify the unsatisfied stored generic bound, got: {err}"
    );
}

#[test]
fn deferred_lambda_alias_chain_resolves_all_moved_bindings() {
    let input = r#"
fun main: () -> List<Int32> = {
    val build = |value| [];
    val build_again = build;
    val maker: Int32 -> List<Int32> = build_again;
    1 |> maker
}
"#;

    type_check(input).expect("lambda alias resolution should finalize the complete moved chain");
}

#[test]
fn deferred_lambda_alias_chain_rejects_source_reuse() {
    let input = r#"
fun main: () -> List<Int32> = {
    val build = |value| [];
    val build_again = build;
    build;
    val maker: Int32 -> List<Int32> = build_again;
    1 |> maker
}
"#;

    let err = type_check(input)
        .expect_err("resolving a lambda alias chain must retain source affine uses");
    assert!(
        err.contains("affine type violation") && err.contains("build"),
        "error should identify the reused lambda source alias, got: {err}"
    );
}

#[test]
fn deferred_branch_callable_alias_chain_resolves_all_moved_bindings() {
    let input = r#"
fun main: (flag: Boolean) -> List<Int32> = {
    val build = flag then {
        |value| []
    } else {
        |value| []
    };
    val build_again = build;
    val maker: Int32 -> List<Int32> = build_again;
    1 |> maker
}
"#;

    type_check(input)
        .expect("branch-callable alias resolution should finalize the complete moved chain");
}

#[test]
fn independent_deferred_lambda_groups_do_not_cross_resolve() {
    let input = r#"
fun main: () -> List<String> = {
    mut val a = |x| [];
    mut val b = |x| () Option::None;
    val same = a == b;
    val maker: Int32 -> List<String> = a;
    1 |> b
}
"#;

    let err = type_check(input)
        .expect_err("resolving one deferred lambda must not resolve an independent lambda");
    assert!(
        !err.contains("affine type violation"),
        "independent mutable lambdas should not share affine state, got: {err}"
    );
    assert!(
        err.contains("Type mismatch")
            || (err.contains("Option") && err.contains("List"))
            || (err.contains("Cannot infer type") && err.contains("None")),
        "error should preserve b's Option-returning body, got: {err}"
    );
}

#[test]
fn deferred_callable_lookup_respects_lexical_shadowing() {
    let input = r#"
fun main: () -> Int32 = {
    val keep = identity;
    val read = |keep: Int32| {
        val x = keep;
        x
    };
    val answer = 41 |> read;
    val outer_id: Int32 -> Int32 = keep;
    answer
}
"#;

    type_check(input)
        .expect("an inner concrete binding must shadow an outer deferred callable completely");
}

#[test]
fn generic_sink_finalization_ignores_shadowing_owner_name() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun plain: (value: Int32) -> Int32 = {
    value
}

fun main: () -> Int32 = {
    mut val base = identity;
    mut val keep = base;
    val read = |keep: Int32| {
        val picked = (base, plain) choose_first;
        keep
    };
    1 |> read
}
"#;

    type_check(input)
        .expect("synthetic group finalization must not resolve a shadowing lambda parameter");
}

#[test]
fn mutable_reassignment_rejects_multi_member_deferred_alias_group() {
    let input = r#"
fun ints: (value: Int32) -> List<Int32> = {
    [value]
}

fun main: (flag: Boolean) -> List<Boolean> = {
    mut val a = |value| [];
    mut val b = a;
    flag then {
        val typed: Int32 -> List<Int32> = b;
        ()
    } else {
        ()
    };
    a = ints;
    val bools: Boolean -> List<Boolean> = b;
    true |> a
}
"#;

    let err = type_check(input)
        .expect_err("reassigning any deferred-origin alias group must be rejected");
    assert!(
        err.contains("reassignment of deferred-origin callable binding")
            && err.contains("not supported in this release"),
        "error should explain the deferred reassignment boundary, got: {err}"
    );
}

#[test]
fn mutable_reassignment_rejects_deferred_alias_owner() {
    let input = r#"
fun ints: (value: Int32) -> List<Int32> = {
    [value]
}

fun main: () -> List<Int32> = {
    mut val source = |value| () Option::None;
    mut val owner = source;
    owner = ints;
    1 |> source
}
"#;

    let err = type_check(input)
        .expect_err("overwriting the deferred owner must not orphan its stored body");
    assert!(
        err.contains("reassignment of deferred-origin callable binding") && err.contains("owner"),
        "error should identify the deferred owner reassignment boundary, got: {err}"
    );
}

#[test]
fn mutable_reassignment_rejects_single_deferred_holder() {
    let input = r#"
fun ints: (value: Int32) -> List<Int32> = {
    [value]
}

fun main: () -> List<Int32> = {
    mut val build = |value| () Option::None;
    build = ints;
    1 |> build
}
"#;

    let err = type_check(input)
        .expect_err("even a single deferred-origin holder cannot be reassigned in this release");
    assert!(
        err.contains("reassignment of deferred-origin callable binding"),
        "error should explain the deferred reassignment boundary, got: {err}"
    );
}

#[test]
fn mutable_reassignment_rejects_resolved_deferred_holder() {
    let input = r#"
fun replacement: (value: Int32) -> Option<String> = {
    ("replacement") Option::Some
}

fun main: () -> Option<String> = {
    mut val build = |value| () Option::None;
    val typed: Int32 -> Option<String> = build;
    build = replacement;
    1 |> build
}
"#;

    let err = type_check(input)
        .expect_err("deferred-origin reassignment remains unsupported after type resolution");
    assert!(
        err.contains("reassignment of deferred-origin callable binding"),
        "error should explain the deferred-origin reassignment boundary, got: {err}"
    );
}

#[test]
fn branch_reassignment_rejects_unresolved_deferred_holder() {
    let input = r#"
fun replacement: (value: Int32) -> List<Int32> = {
    [value]
}

fun main: (flag: Boolean) -> List<Int32> = {
    mut val build = |value| [];
    flag then {
        build = replacement;
        ()
    } else {
        ()
    };
    val typed: Int32 -> List<Int32> = build;
    1 |> typed
}
"#;

    let err = type_check(input)
        .expect_err("branch-local assignment must not replace deferred-origin provenance");
    assert!(
        err.contains("reassignment of deferred-origin callable binding"),
        "error should explain the deferred reassignment boundary, got: {err}"
    );
}

#[test]
fn branch_reassignment_rejects_post_resolution_deferred_alias() {
    let input = r#"
fun replacement: (value: Int32) -> Int32 = {
    value + 1
}

fun main: (flag: Boolean) -> Int32 = {
    mut val source = identity;
    val typed: Int32 -> Int32 = source;
    mut val alias = source;
    flag then {
        alias = replacement;
        ()
    } else {
        ()
    };
    1 |> alias
}
"#;

    let err = type_check(input)
        .expect_err("unannotated aliases must retain deferred-origin reassignment taint");
    assert!(
        err.contains("reassignment of deferred-origin callable binding") && err.contains("alias"),
        "error should identify the post-resolution deferred alias, got: {err}"
    );
}

#[test]
fn then_produced_lambda_infers_from_later_pipe_use() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score + 1
    } else {
        |score| score * 2
    };
    41 |> adjust
}
"#;

    type_check(input).expect("then-produced lambda should infer from later pipe use");
}

#[test]
fn then_produced_lambda_with_prefix_binding_infers_from_later_pipe_use() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val bonus = 1;
        |score| score + bonus
    } else {
        val factor = 2;
        |score| score * factor
    };
    41 |> adjust
}
"#;

    type_check(input).expect("then-produced lambda should infer with replay-safe prefix bindings");
}

#[test]
fn deferred_branch_callable_rejects_mutable_condition_replay() {
    let input = r#"
fun main: () -> Int32 = {
    mut val flag = true;
    val adjust = flag then {
        |value| value + 1
    } else {
        |value| value - 1
    };
    flag = false;
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("a mutable condition cannot be replayed when the deferred callable is used");
    assert!(
        err.contains("replay-safe conditions"),
        "error should explain the deferred replay boundary, got: {err}"
    );
}

#[test]
fn deferred_branch_callable_rejects_outer_binding_capture() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val offset = 1;
    val adjust = flag then {
        |value| value + offset
    } else {
        |value| value - offset
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("deferred branch lambdas cannot replay an outer local capture in this release");
    assert!(
        err.contains("outer binding capture") && err.contains("offset"),
        "error should identify the unsupported outer capture, got: {err}"
    );
}

#[test]
fn match_produced_lambda_infers_from_later_pipe_use() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag match {
        true => { |score| score + 1 }
        false => { |score| score * 2 }
    };
    41 |> adjust
}
"#;

    type_check(input).expect("match-produced lambda should infer from later pipe use");
}

#[test]
fn match_produced_lambda_can_capture_pattern_binding() {
    let input = r#"
fun main: () -> Int32 = {
    val maybe_bonus: Option<Int32> = (2) Option::Some;
    val adjust = maybe_bonus match {
        Some(bonus) => { |score| score + bonus }
        None => { |score| score }
    };
    40 |> adjust
}
"#;

    type_check(input).expect("match-produced lambda should capture pattern bindings during replay");
}

#[test]
fn match_produced_lambda_with_prefix_binding_captures_pattern_binding() {
    let input = r#"
fun main: () -> Int32 = {
    val maybe_bonus: Option<Int32> = (2) Option::Some;
    val adjust = maybe_bonus match {
        Some(bonus) => {
            val doubled = bonus * 2;
            |score| score + doubled
        }
        None => {
            val doubled = 0;
            |score| score + doubled
        }
    };
    38 |> adjust
}
"#;

    type_check(input).expect(
        "match-produced lambda should allow replay-safe prefix bindings from pattern captures",
    );
}

#[test]
fn then_produced_mapper_infers_from_later_map_use() {
    let input = r#"
fun main: (flag: Boolean) -> List<Int32> = {
    val numbers = [1, 2, 3];
    val mapper = flag then {
        |score| score + 1
    } else {
        |score| score * 2
    };
    (numbers, mapper) map
}
"#;

    type_check(input).expect("then-produced mapper should infer from later map use");
}

#[test]
fn branch_produced_lambda_with_prefix_return_mismatch_is_rejected() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val bonus = 1;
        |score| score + bonus
    } else {
        val positive = true;
        |score| score > 0
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("branch lambda return mismatch with prefix bindings should reject");
    assert!(
        err.contains("Type mismatch"),
        "error should explain the return mismatch, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_return_mismatch_is_rejected() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score + 1
    } else {
        |score| score > 0
    };
    41 |> adjust
}
"#;

    let err = type_check(input).expect_err("branch lambda return mismatch should reject");
    assert!(
        err.contains("Type mismatch"),
        "error should explain the return mismatch, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_rejects_non_copy_prefix_binding() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val label = "release";
        |score| score
    } else {
        |score| score
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("deferred callable branch prefix should reject non-Copy bindings");
    assert!(
        err.contains("Copy type"),
        "error should explain the Copy boundary, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_rejects_mutable_prefix_binding() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        mut val bonus = 1;
        |score| score + bonus
    } else {
        |score| score
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("deferred callable branch prefix should reject mutable bindings");
    assert!(
        err.contains("cannot be mutable"),
        "error should explain the mutable prefix boundary, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_rejects_complex_pattern_prefix_binding() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        val Some(bonus): Option<Int32> = (1) Option::Some;
        |score| score + bonus
    } else {
        |score| score
    };
    41 |> adjust
}
"#;

    let err = type_check(input)
        .expect_err("deferred callable branch prefix should reject complex patterns");
    assert!(
        err.contains("simple identifiers"),
        "error should explain the simple identifier prefix boundary, got: {err}"
    );
}

#[test]
fn branch_produced_lambda_arity_mismatch_is_rejected() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score
    } else {
        |score, fallback| score + fallback
    };
    41 |> adjust
}
"#;

    let err = type_check(input).expect_err("branch lambda arity mismatch should reject");
    assert!(
        err.contains("Arity mismatch") || err.contains("Wrong number of arguments"),
        "error should explain the arity mismatch, got: {err}"
    );
}

#[test]
fn unused_branch_produced_lambda_still_requires_expected_type() {
    let input = r#"
fun main: (flag: Boolean) -> Int32 = {
    val adjust = flag then {
        |score| score
    } else {
        |score| score
    };
    0
}
"#;

    let err = type_check(input)
        .expect_err("unresolved branch-produced lambda should reject at scope exit");
    assert!(
        err.contains("Cannot infer type") && err.contains("adjust"),
        "error should identify the unresolved deferred binding, got: {err}"
    );
}

#[test]
fn tail_infers_list_element_type_generically() {
    let input = r#"
fun main: () -> List<String> = {
    val names: List<String> = ["Ada", "Grace", "Edsger"];
    names |> tail
}
"#;

    type_check(input).expect("tail should preserve the concrete List element type");
}

#[test]
fn prelude_map_rejects_non_container_input() {
    let input = r#"
fun main: () -> String = {
    ("Ada", |name| name) map
}
"#;

    let err = type_check(input).expect_err("map should require a Container input");
    assert!(
        err.contains("Container"),
        "error should explain the missing Container form, got: {}",
        err
    );
}

#[test]
fn prelude_map_rejects_result_as_non_container_input() {
    let input = r#"
fun main: () -> Result<Int32, Int32> = {
    val result: Result<Int32, Int32> = (1) Result::Ok;
    (result, |value| value + 1) map
}
"#;

    let err = type_check(input).expect_err("Result is not a v0.0.1 Container");
    assert!(
        err.contains("Container"),
        "error should explain the closed-world Container boundary, got: {err}"
    );
}

#[test]
fn prelude_map_rejects_array_as_non_container_input() {
    let input = r#"
fun main: () -> Array<Int32, 2> = {
    val values: Array<Int32, 2> = [1, 2];
    (values, |value| value + 1) map
}
"#;

    let err = type_check(input).expect_err("Array is not a v0.0.1 Container");
    assert!(
        err.contains("Container"),
        "error should explain the closed-world Container boundary, got: {err}"
    );
}

#[test]
fn prelude_filter_uses_boolean_predicate_context() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n > 1) filter
}
"#;

    type_check(input).expect("filter should infer T and require a Boolean predicate");
}

#[test]
fn prelude_filter_uses_container_item_projection_for_option() {
    let input = r#"
fun main: () -> Option<Int32> = {
    val maybe_value: Option<Int32> = (42) Option::Some
    (maybe_value, |value| value > 10) filter
}
"#;

    type_check(input).expect("filter should infer Option.Item through Container projection");
}

#[test]
fn prelude_filter_rejects_non_boolean_predicate() {
    let input = r#"
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n + 1) filter
}
"#;

    let err = type_check(input).expect_err("filter predicate should return Boolean");
    assert!(
        err.contains("Type mismatch"),
        "error should explain predicate return mismatch, got: {}",
        err
    );
}

#[test]
fn prelude_fold_infers_accumulator_and_item_types() {
    let input = r#"
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, 0, |acc, n| acc + n) fold
}
"#;

    type_check(input).expect("fold should infer both accumulator and item types");
}

#[test]
fn prelude_fold_accepts_multi_parameter_scoped_binder() {
    let input = r#"
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, 0) fold { |total, number|
        total + number
    }
}
"#;

    type_check(input).expect("fold should infer both explicit scoped binders");
}

#[test]
fn prelude_fold_rejects_implicit_unary_focus() {
    let input = r#"
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, 0) fold {
        it + 1
    }
}
"#;

    let err = type_check(input).expect_err("fold needs two explicit focus binders");
    assert!(
        err.contains("implicit focus scopes require a unary function parameter")
            && err.contains("|left, right|"),
        "diagnostic should explain the scoped binder arity mismatch, got: {err}"
    );
}

#[test]
fn prelude_fold_rejects_reducer_return_mismatch() {
    let input = r#"
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, 0, |acc, n| n > acc) fold
}
"#;

    let err = type_check(input).expect_err("fold reducer should return the accumulator type");
    assert!(
        err.contains("Type mismatch"),
        "error should explain reducer return mismatch, got: {}",
        err
    );
}

#[test]
fn generic_lambda_capture_precheck_does_not_consume_affine_value() {
    let input = r#"
fun apply_generic: <T, U>(value: T, f: T -> U) -> U = {
    value |> f
}

fun main: () -> String = {
    val greeting = "hello";
    (1, |n| greeting) apply_generic
}
"#;

    type_check(input).expect("generic lambda pre-check should not consume captured affine values");
}

#[test]
fn generic_lambda_can_appear_before_inference_source() {
    let input = r#"
fun apply_first: <T, U>(f: T -> U, value: T) -> U = {
    value |> f
}

fun main: () -> Int32 = {
    (|n| n + 1, 41) apply_first
}
"#;

    type_check(input).expect("generic inference should not depend on lambda argument order");
}

#[test]
fn generic_empty_list_infers_from_sibling_argument() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> List<Int32> = {
    ([], [1, 2, 3]) choose_first
}
"#;

    type_check(input).expect("empty list should infer through generic sibling constraints");
}

#[test]
fn generic_none_infers_from_sibling_argument() {
    let input = r#"
fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Option<Int32> = {
    (() Option::None, (1) Option::Some) choose_first
}
"#;

    type_check(input).expect("None should infer through generic sibling constraints");
}

#[test]
fn generic_list_elements_infer_from_sibling_constructors() {
    let input = r#"
fun keep_list: <T>(items: List<T>) -> List<T> = {
    items
}

fun main: () -> List<Option<Int32>> = {
    [() Option::None, (1) Option::Some] |> keep_list
}
"#;

    type_check(input).expect("generic list element inference should use sibling constructors");
}

#[test]
fn array_get_accepts_any_length_array_parameter() {
    let input = r#"
fun main: () -> Option<Int32> = {
    ([() Option::None, (1) Option::Some], 0) array_get
}
"#;

    type_check(input)
        .expect("internal array wildcard parameters should accept any concrete array length");
}

#[test]
fn generic_lambda_return_mismatch_is_rejected() {
    let input = r#"
fun main: () -> List<String> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, |n| n * 2) map
}
"#;

    let err = type_check(input).expect_err("expected return type should constrain map result");
    assert!(
        err.contains("Type mismatch"),
        "error should explain type mismatch, got: {}",
        err
    );
}
