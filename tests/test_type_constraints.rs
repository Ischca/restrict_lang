use restrict_lang::type_checker::{format_typed_type, ArrayLength, TypeError, TypedType};
use restrict_lang::type_constraints::{
    contains_infer_var, contains_projection, finalize_type, fresh_type_param_map,
    solve_constraints, solve_constraints_partial_with_initial, solve_constraints_with_forms,
    solve_constraints_with_initial, substitute_type_params, unify, Constraint, ConstraintKind,
    ConstraintOrigin, FormEnvironment, Substitution, TypeVarGenerator,
};

fn origin() -> ConstraintOrigin {
    ConstraintOrigin {
        span: None,
        kind: ConstraintKind::Apply,
    }
}

fn argument_origin(func_name: &str, arg_index: usize) -> ConstraintOrigin {
    ConstraintOrigin {
        span: None,
        kind: ConstraintKind::Argument {
            func_name: func_name.to_string(),
            arg_index,
        },
    }
}

fn return_annotation_origin(var_name: &str) -> ConstraintOrigin {
    ConstraintOrigin {
        span: None,
        kind: ConstraintKind::ReturnAnnotation {
            var_name: var_name.to_string(),
        },
    }
}

fn lambda_return_origin() -> ConstraintOrigin {
    ConstraintOrigin {
        span: None,
        kind: ConstraintKind::LambdaReturn,
    }
}

fn form_bound_origin(type_param: &str) -> ConstraintOrigin {
    ConstraintOrigin {
        span: None,
        kind: ConstraintKind::FormBound {
            type_param: type_param.to_string(),
        },
    }
}

fn assert_user_diagnostic_does_not_expose_inference_internals(message: &str) {
    for internal in ["InferVar", "TypeVarId", "Projection", "?0", "?1", "?2"] {
        assert!(
            !message.contains(internal),
            "diagnostic should hide inference internals ({internal}), got: {message}"
        );
    }
    assert!(
        !message.contains(" as Container."),
        "diagnostic should hide raw associated-type projection syntax, got: {message}"
    );
}

#[test]
fn infer_var_unifies_with_concrete_type() {
    let mut vars = TypeVarGenerator::new();
    let var = vars.fresh_var();
    let mut subst = Substitution::new();

    unify(&var, &TypedType::Int32, &mut subst).expect("InferVar should bind to Int32");

    let finalized = finalize_type(&var, &subst).expect("bound InferVar should finalize");
    assert_eq!(finalized, TypedType::Int32);
}

#[test]
fn function_unification_binds_nested_infer_var() {
    let mut vars = TypeVarGenerator::new();
    let item = vars.fresh_var();
    let expected = TypedType::Function {
        params: vec![TypedType::List(Box::new(item.clone()))],
        return_type: Box::new(item.clone()),
    };
    let actual = TypedType::Function {
        params: vec![TypedType::List(Box::new(TypedType::String))],
        return_type: Box::new(TypedType::String),
    };
    let mut subst = Substitution::new();

    unify(&expected, &actual, &mut subst).expect("nested InferVar should unify");

    assert_eq!(
        finalize_type(&item, &subst).expect("item should finalize"),
        TypedType::String
    );
}

fn box_record(arg: TypedType) -> TypedType {
    TypedType::Record {
        name: "Box".to_string(),
        type_args: vec![arg],
        frozen: false,
        hash: None,
        parent_hash: None,
    }
}

#[test]
fn finalize_type_applies_substitution_inside_record_type_args() {
    let mut vars = TypeVarGenerator::new();
    let item = vars.fresh_var();
    let record = box_record(item.clone());
    let mut subst = Substitution::new();

    unify(&item, &TypedType::String, &mut subst).expect("record item should bind");

    assert_eq!(
        finalize_type(&record, &subst).expect("record type args should finalize"),
        box_record(TypedType::String)
    );
}

#[test]
fn unresolved_record_type_arg_fails_finalization() {
    let mut vars = TypeVarGenerator::new();
    let item = vars.fresh_var();
    let record = box_record(item);
    let subst = Substitution::new();

    let err = finalize_type(&record, &subst).expect_err("record type arg must resolve");
    let message = err.to_string();

    assert!(matches!(err, TypeError::CannotInferType(_)));
    assert!(
        message.contains("Cannot infer type"),
        "diagnostic should report the unresolved record type arg, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn type_error_display_hides_nested_inference_placeholder_detail() {
    let err = TypeError::CannotInferType("Option<?12>".to_string());
    let message = err.to_string();

    assert!(
        message.contains("Cannot infer type"),
        "diagnostic should still explain inference failure, got: {message}"
    );
    assert!(
        message.contains("type annotation") || message.contains("concrete type"),
        "diagnostic should suggest a concrete type context, got: {message}"
    );
    assert!(
        !message.contains("unknown type") && !message.contains("an inferred type"),
        "diagnostic should hide placeholder wording, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn occurs_check_rejects_recursive_record_type_arg() {
    let mut vars = TypeVarGenerator::new();
    let item = vars.fresh_var();
    let mut subst = Substitution::new();

    let err = unify(&item, &box_record(item.clone()), &mut subst)
        .expect_err("recursive record type should be rejected");
    let message = err.to_string();

    assert!(matches!(err, TypeError::CannotInferType(_)));
    assert!(
        message.contains("recursive type"),
        "diagnostic should explain the recursive type, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn projection_inside_record_type_arg_fails_finalization() {
    let record = box_record(TypedType::Projection {
        base: Box::new(TypedType::List(Box::new(TypedType::Int32))),
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        args: vec![TypedType::String],
    });
    let subst = Substitution::new();

    let err = finalize_type(&record, &subst).expect_err("projection must resolve inside records");
    let message = err.to_string();

    assert!(matches!(err, TypeError::UnresolvedProjection(_)));
    assert!(
        message.contains("Cannot resolve generic collection result type"),
        "diagnostic should report unresolved projection, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn array_type_format_uses_spec_surface_not_internal_brackets() {
    let ty = TypedType::Array(
        Box::new(TypedType::Option(Box::new(TypedType::Int32))),
        ArrayLength::Known(3),
    );

    assert_eq!(format_typed_type(&ty), "Array<Option<Int32>, 3>");
}

#[test]
fn internal_array_wildcard_length_unifies_with_any_concrete_length() {
    let mut vars = TypeVarGenerator::new();
    let item = vars.fresh_var();
    let expected = TypedType::Array(Box::new(item.clone()), ArrayLength::AnyInternal);
    let actual = TypedType::Array(Box::new(TypedType::Int32), ArrayLength::Known(3));
    let mut subst = Substitution::new();

    unify(&expected, &actual, &mut subst)
        .expect("internal array wildcard length should accept any concrete length");

    assert_eq!(
        finalize_type(&item, &subst).expect("array item should finalize"),
        TypedType::Int32
    );
}

#[test]
fn public_zero_length_array_does_not_unify_with_nonzero_length() {
    let expected = TypedType::Array(Box::new(TypedType::Int32), ArrayLength::Known(0));
    let actual = TypedType::Array(Box::new(TypedType::Int32), ArrayLength::Known(3));
    let mut subst = Substitution::new();

    let err = unify(&expected, &actual, &mut subst)
        .expect_err("public Array<T, 0> should not act as an internal wildcard");
    let message = err.to_string();

    assert!(
        message.contains("Array<Int32, 0>") && message.contains("Array<Int32, 3>"),
        "diagnostic should keep concrete lengths visible, got: {message}"
    );
}

#[test]
fn unresolved_infer_var_fails_finalization() {
    let mut vars = TypeVarGenerator::new();
    let var = vars.fresh_var();
    let subst = Substitution::new();

    let err = finalize_type(&var, &subst).expect_err("unbound InferVar should not finalize");
    let message = err.to_string();

    assert!(matches!(err, TypeError::CannotInferType(_)));
    assert!(
        message.contains("Cannot infer type"),
        "diagnostic should preserve the actionable inference failure, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn type_params_are_freshened_into_infer_vars() {
    let mut vars = TypeVarGenerator::new();
    let names = vec!["T".to_string(), "U".to_string()];
    let type_vars = fresh_type_param_map(&names, &mut vars);
    let signature = TypedType::Function {
        params: vec![TypedType::List(Box::new(TypedType::TypeParam(
            "T".to_string(),
        )))],
        return_type: Box::new(TypedType::TypeParam("U".to_string())),
    };

    let freshened = substitute_type_params(&signature, &type_vars);

    assert_eq!(format_typed_type(&freshened), "(List<?0>) -> ?1");
}

#[test]
fn raw_type_params_do_not_bind_directly_to_concrete_types() {
    let mut subst = Substitution::new();

    let err = unify(
        &TypedType::TypeParam("T".to_string()),
        &TypedType::Int32,
        &mut subst,
    )
    .expect_err("raw TypeParam should not bind directly");
    let message = err.to_string();

    assert!(
        matches!(err, TypeError::TypeMismatch { .. }),
        "raw TypeParam should remain a declaration type, got: {message}"
    );
    assert!(
        message.contains("expected T, found Int32"),
        "diagnostic should show the unsatisfied raw type parameter, got: {message}"
    );
}

#[test]
fn raw_type_param_constraints_do_not_solve_without_freshening() {
    let constraints = vec![Constraint::TypeEquals {
        expected: TypedType::TypeParam("T".to_string()),
        actual: TypedType::Int32,
        origin: argument_origin("raw_type_param", 0),
    }];

    let err = solve_constraints(&constraints)
        .expect_err("raw TypeParam should not be solved without freshening");
    let message = err.to_string();

    assert!(
        matches!(err, TypeError::TypeMismatch { .. }),
        "raw TypeParam constraint should remain unsolved, got: {message}"
    );
    assert!(
        message.contains("expected T, found Int32"),
        "diagnostic should preserve the raw type parameter mismatch, got: {message}"
    );
}

#[test]
fn solver_applies_return_annotation_constraint() {
    let mut vars = TypeVarGenerator::new();
    let result_item = vars.fresh_var();
    let inferred_return = TypedType::List(Box::new(result_item.clone()));
    let expected_return = TypedType::List(Box::new(TypedType::String));
    let constraints = vec![Constraint::TypeEquals {
        expected: inferred_return,
        actual: expected_return,
        origin: origin(),
    }];

    let subst = solve_constraints(&constraints).expect("return constraint should solve");

    assert_eq!(
        finalize_type(&result_item, &subst).expect("result item should finalize"),
        TypedType::String
    );
}

#[test]
fn solver_extends_existing_substitution() {
    let mut vars = TypeVarGenerator::new();
    let existing = vars.fresh_var();
    let result = vars.fresh_var();
    let mut initial = Substitution::new();
    unify(
        &existing,
        &TypedType::List(Box::new(TypedType::Int32)),
        &mut initial,
    )
    .expect("initial substitution should bind existing variable");
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: existing,
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        type_args: vec![TypedType::String],
        result: result.clone(),
        origin: origin(),
    }];

    let subst =
        solve_constraints_with_initial(&constraints, &initial).expect("solver should extend input");

    assert_eq!(
        finalize_type(&result, &subst).expect("mapped result should finalize"),
        TypedType::List(Box::new(TypedType::String))
    );
}

#[test]
fn projection_is_reported_before_codegen_boundary() {
    let projection = TypedType::Projection {
        base: Box::new(TypedType::List(Box::new(TypedType::Int32))),
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        args: vec![TypedType::String],
    };
    let subst = Substitution::new();

    assert!(contains_projection(&projection));
    assert!(!contains_infer_var(&projection));

    let err = finalize_type(&projection, &subst).expect_err("projection must resolve first");
    let message = err.to_string();
    assert!(matches!(err, TypeError::UnresolvedProjection(_)));
    assert!(
        message.contains("Cannot resolve generic collection result type"),
        "diagnostic should preserve unresolved projection context, got: {message}"
    );
    assert!(
        !message.contains("Mapped"),
        "diagnostic should hide the internal associated type name, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn has_form_accepts_builtin_container_adoptions() {
    let constraints = vec![
        Constraint::HasForm {
            ty: TypedType::List(Box::new(TypedType::Int32)),
            form_name: "Container".to_string(),
            origin: origin(),
        },
        Constraint::HasForm {
            ty: TypedType::Option(Box::new(TypedType::String)),
            form_name: "Container".to_string(),
            origin: origin(),
        },
    ];

    solve_constraints(&constraints).expect("List and Option should adopt Container");
}

#[test]
fn has_form_accepts_known_container_with_unresolved_inner_type() {
    let mut vars = TypeVarGenerator::new();
    let list_item = vars.fresh_var();
    let option_value = vars.fresh_var();
    let constraints = vec![
        Constraint::HasForm {
            ty: TypedType::List(Box::new(list_item.clone())),
            form_name: "Container".to_string(),
            origin: origin(),
        },
        Constraint::HasForm {
            ty: TypedType::Option(Box::new(option_value.clone())),
            form_name: "Container".to_string(),
            origin: origin(),
        },
    ];

    let subst = solve_constraints(&constraints)
        .expect("known container constructors should not wait for inner inference");

    assert!(subst.is_empty());
    assert!(matches!(
        finalize_type(&list_item, &subst),
        Err(TypeError::CannotInferType(_))
    ));
    assert!(matches!(
        finalize_type(&option_value, &subst),
        Err(TypeError::CannotInferType(_))
    ));
}

#[test]
fn has_form_waits_until_infer_var_is_bound() {
    let mut vars = TypeVarGenerator::new();
    let container = vars.fresh_var();
    let constraints = vec![
        Constraint::HasForm {
            ty: container.clone(),
            form_name: "Container".to_string(),
            origin: origin(),
        },
        Constraint::TypeEquals {
            expected: container,
            actual: TypedType::List(Box::new(TypedType::Int32)),
            origin: origin(),
        },
    ];

    solve_constraints(&constraints).expect("form solving should wait for concrete base type");
}

#[test]
fn has_form_rejects_non_adopted_type() {
    let constraints = vec![Constraint::HasForm {
        ty: TypedType::String,
        form_name: "Container".to_string(),
        origin: origin(),
    }];

    let err = solve_constraints(&constraints).expect_err("String is not a Container");
    let message = err.to_string();
    assert!(matches!(err, TypeError::UnsupportedFeature(_)));
    assert!(
        message.contains("built-in Container constraint"),
        "error should not imply user-defined forms are current syntax, got: {message}"
    );
}

#[test]
fn supplied_empty_form_environment_rejects_container_projection() {
    let empty_forms = FormEnvironment::new();
    let has_form_constraints = vec![Constraint::HasForm {
        ty: TypedType::List(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        origin: origin(),
    }];
    let has_form_err = solve_constraints_with_forms(&has_form_constraints, &empty_forms)
        .expect_err("empty form environment should not adopt List as Container");
    assert!(
        has_form_err.to_string().contains("Container"),
        "diagnostic should mention missing Container adoption, got: {has_form_err}"
    );

    let mut vars = TypeVarGenerator::new();
    let mapped = vars.fresh_var();
    let projection_constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::List(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        type_args: vec![TypedType::String],
        result: mapped,
        origin: origin(),
    }];
    let projection_err = solve_constraints_with_forms(&projection_constraints, &empty_forms)
        .expect_err("empty form environment should not resolve List.Container.Mapped");
    assert!(
        projection_err.to_string().contains("Container"),
        "diagnostic should mention missing Container projection, got: {projection_err}"
    );
}

#[test]
fn associated_type_resolution_maps_list_container() {
    let mut vars = TypeVarGenerator::new();
    let result = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::List(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        type_args: vec![TypedType::String],
        result: result.clone(),
        origin: origin(),
    }];

    let subst = solve_constraints(&constraints).expect("List.Mapped<String> should resolve");

    assert_eq!(
        finalize_type(&result, &subst).expect("mapped result should finalize"),
        TypedType::List(Box::new(TypedType::String))
    );
}

#[test]
fn associated_type_resolution_maps_option_container() {
    let mut vars = TypeVarGenerator::new();
    let result = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::Option(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        type_args: vec![TypedType::String],
        result: result.clone(),
        origin: origin(),
    }];

    let subst = solve_constraints(&constraints).expect("Option.Mapped<String> should resolve");

    assert_eq!(
        finalize_type(&result, &subst).expect("mapped result should finalize"),
        TypedType::Option(Box::new(TypedType::String))
    );
}

#[test]
fn associated_type_resolution_waits_for_base_type() {
    let mut vars = TypeVarGenerator::new();
    let base = vars.fresh_var();
    let result = vars.fresh_var();
    let constraints = vec![
        Constraint::AssociatedTypeResolution {
            base_type: base.clone(),
            form_name: "Container".to_string(),
            assoc_name: "Mapped".to_string(),
            type_args: vec![TypedType::String],
            result: result.clone(),
            origin: origin(),
        },
        Constraint::TypeEquals {
            expected: base,
            actual: TypedType::List(Box::new(TypedType::Int32)),
            origin: origin(),
        },
    ];

    let subst = solve_constraints(&constraints).expect("projection should wait for base inference");

    assert_eq!(
        finalize_type(&result, &subst).expect("mapped result should finalize"),
        TypedType::List(Box::new(TypedType::String))
    );
}

#[test]
fn associated_type_item_resolves_container_element() {
    let mut vars = TypeVarGenerator::new();
    let item = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::List(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        assoc_name: "Item".to_string(),
        type_args: vec![],
        result: item.clone(),
        origin: origin(),
    }];

    let subst = solve_constraints(&constraints).expect("List.Item should resolve");

    assert_eq!(
        finalize_type(&item, &subst).expect("item result should finalize"),
        TypedType::Int32
    );
}

#[test]
fn associated_type_item_resolves_list_with_unresolved_item() {
    let mut vars = TypeVarGenerator::new();
    let item = vars.fresh_var();
    let result = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::List(Box::new(item.clone())),
        form_name: "Container".to_string(),
        assoc_name: "Item".to_string(),
        type_args: vec![],
        result: result.clone(),
        origin: origin(),
    }];

    let subst = solve_constraints(&constraints)
        .expect("List<?T>.Item should resolve to ?T without waiting");

    assert_eq!(
        subst.apply(&result).expect("result should be substituted"),
        item
    );
}

#[test]
fn associated_type_value_resolves_option_with_unresolved_value() {
    let mut vars = TypeVarGenerator::new();
    let value = vars.fresh_var();
    let result = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::Option(Box::new(value.clone())),
        form_name: "Container".to_string(),
        assoc_name: "Value".to_string(),
        type_args: vec![],
        result: result.clone(),
        origin: origin(),
    }];

    let subst = solve_constraints(&constraints)
        .expect("Option<?T>.Value should resolve to ?T without waiting");

    assert_eq!(
        subst.apply(&result).expect("result should be substituted"),
        value
    );
}

#[test]
fn associated_type_item_rejects_type_arguments() {
    let mut vars = TypeVarGenerator::new();
    let result = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::List(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        assoc_name: "Item".to_string(),
        type_args: vec![TypedType::String],
        result,
        origin: origin(),
    }];

    let err = solve_constraints(&constraints).expect_err("Item should not accept type arguments");
    let message = err.to_string();

    assert!(
        message.contains("Cannot resolve generic collection result type"),
        "unexpected error: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn associated_type_mapped_requires_exactly_one_type_argument() {
    let mut vars = TypeVarGenerator::new();
    let result = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::Option(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        type_args: vec![],
        result,
        origin: origin(),
    }];

    let err = solve_constraints(&constraints).expect_err("Mapped should require one type argument");
    let message = err.to_string();

    assert!(
        message.contains("Cannot resolve generic collection result type"),
        "unexpected error: {message}"
    );
    assert!(
        !message.contains("Mapped"),
        "diagnostic should hide internal associated type names, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn associated_type_resolution_rejects_unknown_assoc_name() {
    let mut vars = TypeVarGenerator::new();
    let result = vars.fresh_var();
    let constraints = vec![Constraint::AssociatedTypeResolution {
        base_type: TypedType::Option(Box::new(TypedType::Int32)),
        form_name: "Container".to_string(),
        assoc_name: "Missing".to_string(),
        type_args: vec![],
        result,
        origin: origin(),
    }];

    let err = solve_constraints(&constraints).expect_err("unknown associated type should fail");
    let message = err.to_string();

    assert!(
        message.contains("Cannot resolve generic collection result type"),
        "unexpected error: {message}"
    );
    assert!(
        !message.contains("Missing"),
        "diagnostic should hide internal associated type names, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn bare_infer_form_and_projection_targets_still_defer() {
    let mut vars = TypeVarGenerator::new();
    let form_target = vars.fresh_var();
    let projection_base = vars.fresh_var();
    let result = vars.fresh_var();
    let initial = Substitution::new();
    let constraints = vec![
        Constraint::HasForm {
            ty: form_target.clone(),
            form_name: "Container".to_string(),
            origin: origin(),
        },
        Constraint::AssociatedTypeResolution {
            base_type: projection_base.clone(),
            form_name: "Container".to_string(),
            assoc_name: "Item".to_string(),
            type_args: vec![],
            result,
            origin: origin(),
        },
    ];

    let subst = solve_constraints_partial_with_initial(&constraints, &initial)
        .expect("bare inference variables should remain deferred in partial solving");

    assert!(subst.is_empty());
    assert_eq!(
        subst
            .apply(&form_target)
            .expect("form target should remain unbound"),
        form_target
    );
    assert_eq!(
        subst
            .apply(&projection_base)
            .expect("projection base should remain unbound"),
        projection_base
    );
}

#[test]
fn type_mismatch_reports_argument_origin() {
    let constraints = vec![Constraint::TypeEquals {
        expected: TypedType::Int32,
        actual: TypedType::String,
        origin: argument_origin("score", 1),
    }];

    let err = solve_constraints(&constraints).expect_err("argument mismatch should fail");
    let message = err.to_string();

    assert!(
        message.contains("Type mismatch"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("argument 2 of score"),
        "unexpected error: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn unresolved_projection_reports_return_annotation_origin() {
    let projection = TypedType::Projection {
        base: Box::new(TypedType::List(Box::new(TypedType::Int32))),
        form_name: "Container".to_string(),
        assoc_name: "Mapped".to_string(),
        args: vec![TypedType::String],
    };
    let constraints = vec![Constraint::TypeEquals {
        expected: projection,
        actual: TypedType::List(Box::new(TypedType::String)),
        origin: return_annotation_origin("main"),
    }];

    let err = solve_constraints(&constraints).expect_err("projection should remain unresolved");
    let message = err.to_string();

    assert!(
        message.contains("Cannot resolve generic collection result type"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("return annotation of main"),
        "unexpected error: {message}"
    );
    assert!(
        !message.contains("Mapped"),
        "diagnostic should hide the internal associated type name, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn type_mismatch_reports_lambda_return_origin() {
    let constraints = vec![Constraint::TypeEquals {
        expected: TypedType::Boolean,
        actual: TypedType::Int32,
        origin: lambda_return_origin(),
    }];

    let err = solve_constraints(&constraints).expect_err("lambda return mismatch should fail");
    let message = err.to_string();

    assert!(
        message.contains("lambda return"),
        "unexpected error: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn unresolved_form_reports_form_bound_origin() {
    let mut vars = TypeVarGenerator::new();
    let constraints = vec![Constraint::HasForm {
        ty: vars.fresh_var(),
        form_name: "Container".to_string(),
        origin: form_bound_origin("T"),
    }];

    let err = solve_constraints(&constraints).expect_err("unbound form constraint should fail");
    let message = err.to_string();

    assert!(
        message.contains("form bound of T"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("built-in Container constraint"),
        "error should identify Container as a built-in constraint, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn format_inference_internal_types_for_diagnostics() {
    let mut vars = TypeVarGenerator::new();
    let var = vars.fresh_var();

    assert_eq!(format_typed_type(&var), "?0");
}

#[test]
fn type_error_display_sanitizes_direct_internal_type_text() {
    let err = TypeError::TypeMismatch {
        expected: "List<?0>".to_string(),
        found: "InferVar(TypeVarId(0))".to_string(),
    };
    let message = err.to_string();

    assert!(
        message.contains("Type mismatch"),
        "diagnostic should preserve mismatch context, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}

#[test]
fn type_error_display_sanitizes_direct_projection_text() {
    let err = TypeError::UnresolvedProjection(
        "List<?0> as Container.Mapped<String> (Projection)".to_string(),
    );
    let message = err.to_string();

    assert!(
        message.contains("Cannot resolve generic collection result type"),
        "diagnostic should preserve associated type context, got: {message}"
    );
    assert!(
        !message.contains("Mapped"),
        "diagnostic should hide the internal associated type name, got: {message}"
    );
    assert_user_diagnostic_does_not_expose_inference_internals(&message);
}
