//! Tests for the Checked IR builder.

use super::*;
use crate::ir::layout::LayoutId;
use crate::ir::{HostAbi, InternalOnlyReason, ScalarRepr};
use crate::parser::parse_program;
use crate::type_checker::TypedType;

fn checked_ir(source: &str) -> CheckedProgramIr {
    let (remaining, program) = parse_program(source).expect("source should parse");
    assert!(remaining.trim().is_empty(), "unparsed input: {remaining:?}");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("source should type-check");
    build_checked_ir(&program, &checker).expect("checked IR should build")
}

fn scalar_test_lowering_summary() -> CheckedFunctionLoweringSummary {
    CheckedFunctionLoweringSummary {
        source_exported: false,
        declared_type_params: Vec::new(),
        temporal_constraints: Vec::new(),
        param_host_abis: Vec::new(),
        return_host_abi: HostAbi::Scalar(ScalarRepr::I32),
        body_result: None,
        required_layouts: Vec::new(),
        readiness: LoweringReadiness {
            v001_host_abi_eligible: true,
            internal_layout_ready: false,
            host_abi_blockers: Vec::new(),
            internal_lowering_blockers: vec![InternalLoweringBlocker::MissingBodyResult],
        },
    }
}

#[test]
fn builder_collects_checked_function_signature() {
    let ir = checked_ir(
        r#"
fun add_one: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    );

    assert_eq!(ir.functions.len(), 1);
    let function = &ir.functions[0];
    assert_eq!(function.name, "add_one");
    assert_eq!(function.params.len(), 1);
    assert_eq!(function.params[0].name, "value");
    assert_eq!(function.return_type.as_typed_type(), &TypedType::Int32);
    assert!(!function.typed_exprs.is_empty());
    assert!(function.monomorphic);
}

#[test]
fn builder_keeps_generic_signature_non_monomorphic() {
    let ir = checked_ir(
        r#"
fun identity: <T>(value: T) -> T = {
    value
}
"#,
    );

    let function = &ir.functions[0];
    assert_eq!(
        function.return_type.as_typed_type(),
        &TypedType::TypeParam("T".to_string())
    );
    assert!(!function.monomorphic);
}

#[test]
fn builder_keeps_open_generic_record_fields_opaque() {
    let ir = checked_ir(
        r#"
record Box<T> {
    value: T
}

fun keep_box: <T>(item: Box<T>) -> Box<T> = {
    item
}
"#,
    );

    let function = &ir.functions[0];
    let ValueRepr::Ref(layout_id) = function.return_repr else {
        panic!("Box<T> should use a typed ref representation");
    };
    let descriptor = ir
        .layout_table
        .get(layout_id)
        .expect("record layout should be present");
    let LayoutKind::Record(layout) = &descriptor.kind else {
        panic!("expected Record layout descriptor");
    };

    assert_eq!(layout.name, "Box");
    assert_eq!(layout.type_args, vec!["T"]);
    assert!(layout.fields.is_empty());
    assert!(!function.monomorphic);
    assert!(function.lowering.required_layouts.contains(&layout_id));
    assert!(!function.lowering.readiness.v001_host_abi_eligible);
}

#[test]
fn builder_records_param_binding_provenance() {
    let ir = checked_ir(
        r#"
fun main: (items: List<Int32>) -> List<Int32> = {
    items
}
"#,
    );

    let function = &ir.functions[0];
    let param_binding = function.params[0].binding;

    assert_eq!(function.bindings.len(), 1);
    assert_eq!(function.bindings[0].id, param_binding);
    assert_eq!(function.bindings[0].name, "items");
    assert_eq!(
        function.bindings[0].source,
        CheckedBindingSource::Param { index: 0 }
    );
    assert!(function.bindings[0].value.is_none());
    assert!(function.typed_exprs.iter().any(|expr| {
        matches!(expr.kind, TypedExprKind::Binding(binding) if binding == param_binding)
    }));
}

#[test]
fn builder_records_local_identifier_binding_provenance() {
    let ir = checked_ir(
        r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: (items: List<Int32>) -> List<Int32> = {
    val alias = items
    alias |> keep
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let alias = main
        .bindings
        .iter()
        .find(|binding| binding.name == "alias")
        .expect("local alias binding should be present");

    assert_eq!(alias.source, CheckedBindingSource::Local);
    assert!(!alias.mutable);
    assert!(alias.value.is_some());

    let pipe_site = main
        .apply_sites
        .iter()
        .find(|site| site.apply.flavor == ApplyFlavor::Pipe)
        .expect("pipe apply should be present");
    let moved_binding_expr = main
        .typed_exprs
        .iter()
        .find(|expr| expr.value == Some(pipe_site.apply.args[0]))
        .expect("pipe argument producer should be present");

    assert!(matches!(
        moved_binding_expr.kind,
        TypedExprKind::Binding(binding) if binding == alias.id
    ));
}

#[test]
fn builder_records_exported_scalar_lowering_readiness() {
    let ir = checked_ir(
        r#"
export fun public_score: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    );

    let function = &ir.functions[0];
    let summary = &function.lowering;

    assert!(summary.source_exported);
    assert!(summary.declared_type_params.is_empty());
    assert_eq!(
        summary.param_host_abis,
        vec![HostAbi::Scalar(ScalarRepr::I32)]
    );
    assert_eq!(summary.return_host_abi, HostAbi::Scalar(ScalarRepr::I32));
    assert!(summary.body_result.is_some());
    assert!(summary.readiness.v001_host_abi_eligible);
    assert!(summary.readiness.internal_layout_ready);
    assert!(summary.readiness.host_abi_blockers.is_empty());
    assert!(summary.readiness.internal_lowering_blockers.is_empty());
}

#[test]
fn builder_blocks_declared_generic_signature_from_v001_host_abi() {
    let ir = checked_ir(
        r#"
fun tagged: <T>(value: Int32) -> Int32 = {
    value
}
"#,
    );

    let function = &ir.functions[0];

    assert!(function.monomorphic);
    assert_eq!(function.lowering.declared_type_params, vec!["T"]);
    assert!(!function.lowering.readiness.v001_host_abi_eligible);
    assert!(function
        .lowering
        .readiness
        .host_abi_blockers
        .contains(&HostAbiBlocker::DeclaredTypeParam("T".to_string())));
}

#[test]
fn builder_keeps_composite_layout_internal_only() {
    let ir = checked_ir(
        r#"
fun keep_scores: (items: List<Int32>) -> List<Int32> = {
    items
}
"#,
    );

    let function = &ir.functions[0];
    let composite = InternalOnlyReason::CompositeHostAbiUnstable;

    assert!(!function.lowering.readiness.v001_host_abi_eligible);
    assert!(function.lowering.readiness.internal_layout_ready);
    assert!(!function.lowering.required_layouts.is_empty());
    assert_eq!(function.params[0].repr, function.return_repr);
    let body_result = function
        .lowering
        .body_result
        .expect("composite function should record a body result");
    let body_producer = function
        .typed_exprs
        .iter()
        .find(|expr| expr.flow.produced().contains(&body_result))
        .expect("body result should have a typed producer");
    assert_eq!(body_producer.repr, function.return_repr);
    assert_eq!(
        function.lowering.param_host_abis,
        vec![HostAbi::InternalOnly(composite.clone())]
    );
    assert_eq!(
        function.lowering.return_host_abi,
        HostAbi::InternalOnly(composite.clone())
    );
    assert!(function.lowering.readiness.host_abi_blockers.contains(
        &HostAbiBlocker::ParamInternalOnly {
            name: "items".to_string(),
            reason: composite.clone(),
        }
    ));
    assert!(function
        .lowering
        .readiness
        .host_abi_blockers
        .contains(&HostAbiBlocker::ReturnInternalOnly { reason: composite }));
}

#[test]
fn builder_records_range_layout_for_lowering_readiness() {
    let ir = checked_ir(
        r#"
fun main: () -> Range<Int32> = {
    [1..10]
}
"#,
    );

    let function = &ir.functions[0];
    let ValueRepr::Ref(layout_id) = function.return_repr else {
        panic!("Range<Int32> should use a typed ref representation");
    };
    let descriptor = ir
        .layout_table
        .get(layout_id)
        .expect("range layout should be present");
    let LayoutKind::Range(layout) = &descriptor.kind else {
        panic!("expected Range layout descriptor");
    };

    assert_eq!(layout.start_offset, 0);
    assert_eq!(layout.end_offset, 4);
    assert_eq!(layout.size, 8);
    assert_eq!(layout.align, 4);
    assert!(function.lowering.required_layouts.contains(&layout_id));
    assert!(!function.lowering.readiness.v001_host_abi_eligible);
    assert!(function.lowering.readiness.internal_layout_ready);
}

#[test]
fn builder_records_source_record_field_layouts_without_host_abi() {
    let ir = checked_ir(
        r#"
record ReleaseScore {
    value: Int32,
    label: String
}

export fun keep_score: (score: ReleaseScore) -> ReleaseScore = {
    score
}
"#,
    );

    let function = &ir.functions[0];
    let ValueRepr::Ref(layout_id) = function.return_repr else {
        panic!("ReleaseScore should use a typed ref representation");
    };
    assert_eq!(function.params[0].repr, function.return_repr);
    let descriptor = ir
        .layout_table
        .get(layout_id)
        .expect("record layout should be present");
    let LayoutKind::Record(layout) = &descriptor.kind else {
        panic!("expected Record layout descriptor");
    };
    assert_eq!(layout.name, "ReleaseScore");
    assert_eq!(layout.fields.len(), 2);
    assert_eq!(layout.fields[0].name, "value");
    assert_eq!(layout.fields[0].offset, 0);
    assert_eq!(
        layout.fields[0].element.repr,
        ValueRepr::Scalar(ScalarRepr::I32)
    );
    assert_eq!(layout.fields[1].name, "label");
    assert_eq!(layout.fields[1].offset, 4);
    let ValueRepr::Ref(string_layout) = layout.fields[1].element.repr else {
        panic!("String field should require a nested layout");
    };

    let composite = InternalOnlyReason::CompositeHostAbiUnstable;
    assert!(function.lowering.required_layouts.contains(&layout_id));
    assert!(function.lowering.required_layouts.contains(&string_layout));
    assert!(function.lowering.source_exported);
    assert!(!function.lowering.readiness.v001_host_abi_eligible);
    assert!(function.lowering.readiness.internal_layout_ready);
    assert_eq!(
        function.lowering.param_host_abis,
        vec![HostAbi::InternalOnly(composite.clone())]
    );
    assert_eq!(
        function.lowering.return_host_abi,
        HostAbi::InternalOnly(composite.clone())
    );
    assert!(function.lowering.readiness.host_abi_blockers.contains(
        &HostAbiBlocker::ParamInternalOnly {
            name: "score".to_string(),
            reason: composite.clone(),
        }
    ));
    assert!(function
        .lowering
        .readiness
        .host_abi_blockers
        .contains(&HostAbiBlocker::ReturnInternalOnly { reason: composite }));
}

#[test]
fn builder_keeps_scalar_sum_layouts_internal_only_for_host_abi() {
    let ir = checked_ir(
        r#"
fun keep_option: (value: Option<Int32>) -> Option<Int32> = {
    value
}

fun keep_result: (value: Result<Int32, Int32>) -> Result<Int32, Int32> = {
    value
}
"#,
    );

    let composite = InternalOnlyReason::CompositeHostAbiUnstable;
    for function in &ir.functions {
        assert!(
            !function.lowering.readiness.v001_host_abi_eligible,
            "{} should not become host ABI eligible from scalar sum candidates",
            function.name
        );
        assert!(function.lowering.readiness.internal_layout_ready);
        assert_eq!(
            function.lowering.param_host_abis,
            vec![HostAbi::InternalOnly(composite.clone())]
        );
        assert_eq!(
            function.lowering.return_host_abi,
            HostAbi::InternalOnly(composite.clone())
        );
    }
}

#[test]
fn builder_records_body_result_producer_for_lowering() {
    let ir = checked_ir(
        r#"
fun add_one: (value: Int32) -> Int32 = {
    value + 1
}
"#,
    );

    let function = &ir.functions[0];
    let body_result = function
        .lowering
        .body_result
        .expect("non-unit function should record a body result");
    let producer = function
        .typed_exprs
        .iter()
        .find(|expr| expr.flow.produced().contains(&body_result))
        .expect("body result should have a typed producer");

    assert_eq!(
        producer.final_type.as_typed_type(),
        function.return_type.as_typed_type()
    );
    assert_eq!(producer.repr, function.return_repr);
}

#[test]
fn builder_rejects_stale_lowering_layout_summary() {
    let mut ir = checked_ir(
        r#"
fun keep_scores: (items: List<Int32>) -> List<Int32> = {
    items
}
"#,
    );

    ir.functions[0]
        .lowering
        .required_layouts
        .push(LayoutId(999));

    assert!(matches!(
        ir.validate_lowering_summaries(),
        Err(IrBuildError::LoweringInvariantViolation(_))
    ));
}

#[test]
fn builder_normalizes_tuple_and_pipe_applies() {
    let ir = checked_ir(
        r#"
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun inc: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    val value = (1, 2) add
    value |> inc
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let flavors = main
        .apply_sites
        .iter()
        .map(|site| site.apply.flavor)
        .collect::<Vec<_>>();

    assert_eq!(flavors, vec![ApplyFlavor::TupleCall, ApplyFlavor::Pipe]);
    assert_eq!(main.apply_sites[0].callee_hint.as_deref(), Some("add"));
    assert_eq!(main.apply_sites[1].callee_hint.as_deref(), Some("inc"));
    let callee_names = main
        .apply_sites
        .iter()
        .map(|site| match &site.apply.callee_provenance {
            CalleeProvenance::TopLevelFunction(callee) => callee.name.as_str(),
            CalleeProvenance::Value => "<value>",
        })
        .collect::<Vec<_>>();
    assert_eq!(callee_names, vec!["add", "inc"]);
    assert_eq!(main.apply_sites[0].source_index, 0);
    assert_eq!(main.apply_sites[1].source_index, 1);
}

#[test]
fn builder_records_top_level_callee_signature_provenance() {
    let ir = checked_ir(
        r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: () -> List<Int32> = {
    [] |> keep
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let site = main
        .apply_sites
        .iter()
        .find(|site| site.apply.flavor == ApplyFlavor::Pipe)
        .expect("pipe apply should be present");
    let CalleeProvenance::TopLevelFunction(callee) = &site.apply.callee_provenance else {
        panic!("top-level pipe target should record function callee provenance");
    };

    assert_eq!(callee.name, "keep");
    assert!(callee.declared_type_params.is_empty());
    assert_eq!(callee.params.len(), 1);
    assert_eq!(
        callee.params[0].as_typed_type(),
        &TypedType::List(Box::new(TypedType::Int32))
    );
    assert_eq!(
        callee.return_type.as_typed_type(),
        &TypedType::List(Box::new(TypedType::Int32))
    );
    let keep = ir
        .functions
        .iter()
        .find(|function| function.name == "keep")
        .expect("keep IR should be present");
    assert_eq!(callee.return_repr, keep.return_repr);
    assert!(callee.return_repr.is_runtime_reference());
    assert!(callee.monomorphic);
}

#[test]
fn builder_keeps_immediate_lambda_callee_as_value_provenance() {
    let ir = checked_ir(
        r#"
fun main: () -> Int32 = {
    41 |> (|value| value + 1)
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let site = main
        .apply_sites
        .iter()
        .find(|site| site.apply.flavor == ApplyFlavor::Pipe)
        .expect("pipe apply should be present");

    assert!(site.callee_hint.is_none());
    assert!(matches!(
        site.apply.callee_provenance,
        CalleeProvenance::Value
    ));
}

#[test]
fn builder_normalizes_unit_call() {
    let ir = checked_ir(
        r#"
fun seed: () -> Int32 = {
    41
}

fun main: () -> Int32 = {
    () seed
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    assert_eq!(main.apply_sites[0].apply.flavor, ApplyFlavor::UnitCall);
    assert_eq!(main.apply_sites[0].apply.args.len(), 0);
}

#[test]
fn builder_normalizes_grouped_single_direct_call_as_tuple_call() {
    let ir = checked_ir(
        r#"
fun inc: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    (41) inc
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    assert_eq!(main.apply_sites[0].apply.flavor, ApplyFlavor::TupleCall);
    assert_eq!(main.apply_sites[0].apply.args.len(), 1);
    assert_eq!(main.apply_sites[0].callee_hint.as_deref(), Some("inc"));
}

#[test]
fn builder_collects_checked_typed_expr_skeleton() {
    let ir = checked_ir(
        r#"
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun inc: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    val value = (1, 2) add
    value |> inc
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let apply_exprs = main
        .typed_exprs
        .iter()
        .filter(|expr| matches!(expr.kind, TypedExprKind::Apply(_)))
        .collect::<Vec<_>>();

    assert_eq!(apply_exprs.len(), 2);
    assert_eq!(main.apply_sites.len(), apply_exprs.len());
    for site in &main.apply_sites {
        let expr = main
            .typed_exprs
            .iter()
            .find(|expr| expr.id == site.expr_id)
            .expect("apply site should point at a typed expression");
        let TypedExprKind::Apply(apply) = &expr.kind else {
            panic!("apply site should point at a typed apply expression");
        };
        assert_eq!(&site.apply, apply);
        assert_eq!(expr.value, Some(site.apply.result));
    }
    assert!(apply_exprs
        .iter()
        .all(|expr| expr.final_type.as_typed_type() == &TypedType::Int32));
    assert!(main
        .typed_exprs
        .iter()
        .all(|expr| expr.validate_for_codegen().is_ok()));
}

#[test]
fn builder_rejects_apply_site_without_typed_apply_expr() {
    let apply = ApplyIr {
        flavor: ApplyFlavor::TupleCall,
        callee: ValueId(0),
        callee_provenance: CalleeProvenance::Value,
        args: vec![ValueId(1)],
        result: ValueId(2),
    };
    let function = CheckedFunctionIr {
        name: "main".to_string(),
        params: Vec::new(),
        return_type: FinalType::new(TypedType::Int32).unwrap(),
        return_repr: ValueRepr::Scalar(ScalarRepr::I32),
        bindings: Vec::new(),
        apply_sites: vec![NormalizedApplySite {
            source_index: 0,
            expr_id: ExprId(9),
            callee_hint: Some("add".to_string()),
            apply,
        }],
        typed_exprs: Vec::new(),
        monomorphic: true,
        lowering: scalar_test_lowering_summary(),
    };

    assert!(matches!(
        function.validate_shadow_invariants(),
        Err(IrBuildError::ShadowInvariantViolation(_))
    ));
}

#[test]
fn builder_rejects_binding_expr_referencing_missing_binding() {
    let ir = checked_ir(
        r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}
"#,
    );

    let mut function = ir
        .functions
        .iter()
        .find(|function| function.name == "keep")
        .expect("keep IR should be present")
        .clone();
    let binding_expr = function
        .typed_exprs
        .iter_mut()
        .find(|expr| matches!(expr.kind, TypedExprKind::Binding(_)))
        .expect("keep should read its parameter binding");
    binding_expr.kind = TypedExprKind::Binding(BindingId(999));

    assert!(matches!(
        function.validate_shadow_invariants(),
        Err(IrBuildError::ShadowInvariantViolation(_))
    ));
}

#[test]
fn builder_rejects_apply_site_source_index_mismatch() {
    let ir = checked_ir(
        r#"
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> Int32 = {
    (1, 2) add
}
"#,
    );

    let mut function = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present")
        .clone();
    function.apply_sites[0].source_index = 3;

    assert!(matches!(
        function.validate_shadow_invariants(),
        Err(IrBuildError::ShadowInvariantViolation(_))
    ));
}

#[test]
fn builder_reuses_argument_values_in_apply_ir() {
    let ir = checked_ir(
        r#"
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun inc: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    val value = (1, 2) add
    value |> inc
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let literal_values = main
        .typed_exprs
        .iter()
        .filter(|expr| matches!(expr.kind, TypedExprKind::Literal))
        .map(|expr| expr.value.expect("literal should produce a value"))
        .collect::<Vec<_>>();

    assert_eq!(literal_values.len(), 2);
    assert_eq!(main.apply_sites[0].apply.flavor, ApplyFlavor::TupleCall);
    assert_eq!(main.apply_sites[0].apply.args, literal_values);
    let tuple_expr = main
        .typed_exprs
        .iter()
        .find(|expr| expr.id == main.apply_sites[0].expr_id)
        .expect("tuple apply expression should be present");
    assert_eq!(
        tuple_expr.flow.produced(),
        &[main.apply_sites[0].apply.result]
    );
    assert_eq!(
        tuple_expr
            .flow
            .uses()
            .iter()
            .map(|event| (event.value, event.kind, event.at))
            .collect::<Vec<_>>(),
        literal_values
            .iter()
            .copied()
            .map(|value| (value, UseKind::ReadCopy, tuple_expr.id))
            .collect::<Vec<_>>()
    );

    let pipe_site = main
        .apply_sites
        .iter()
        .find(|site| site.apply.flavor == ApplyFlavor::Pipe)
        .expect("pipe apply should be present");
    let pipe_expr_index = main
        .typed_exprs
        .iter()
        .position(|expr| expr.id == pipe_site.expr_id)
        .expect("pipe apply expression should be present");
    let pipe_object_value = main.typed_exprs[..pipe_expr_index]
        .iter()
        .rev()
        .find(|expr| expr.value == Some(pipe_site.apply.args[0]))
        .and_then(|expr| expr.value)
        .expect("pipe object should produce a value before the pipe apply");

    assert_eq!(pipe_site.apply.args, vec![pipe_object_value]);
    let pipe_expr = &main.typed_exprs[pipe_expr_index];
    assert_eq!(pipe_expr.flow.produced(), &[pipe_site.apply.result]);
    assert_eq!(
        pipe_expr
            .flow
            .uses()
            .iter()
            .map(|event| (event.value, event.kind, event.at))
            .collect::<Vec<_>>(),
        vec![(pipe_object_value, UseKind::ReadCopy, pipe_expr.id)]
    );
}

#[test]
fn builder_marks_composite_apply_arguments_as_moves() {
    let ir = checked_ir(
        r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: () -> List<Int32> = {
    [] |> keep
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let site = main
        .apply_sites
        .iter()
        .find(|site| site.apply.flavor == ApplyFlavor::Pipe)
        .expect("pipe apply should be present");
    let expr = main
        .typed_exprs
        .iter()
        .find(|expr| expr.id == site.expr_id)
        .expect("pipe apply expression should be present");

    assert_eq!(site.apply.args.len(), 1);
    assert_eq!(
        expr.flow
            .uses()
            .iter()
            .map(|event| (event.value, event.kind, event.at))
            .collect::<Vec<_>>(),
        vec![(site.apply.args[0], UseKind::Move, expr.id)]
    );
}

#[test]
fn builder_uses_contextual_checked_fact_for_collection_literal() {
    let ir = checked_ir(
        r#"
fun main: () -> List<Int32> = {
    []
}
"#,
    );

    let main = ir
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main IR should be present");
    let list_expr = main
        .typed_exprs
        .iter()
        .find(|expr| matches!(expr.kind, TypedExprKind::Literal))
        .expect("contextual list literal should produce a typed expr");

    assert_eq!(
        list_expr.final_type.as_typed_type(),
        &TypedType::List(Box::new(TypedType::Int32))
    );
    assert!(matches!(list_expr.repr, ValueRepr::Ref(_)));
    assert!(list_expr.validate_for_codegen().is_ok());
}

#[test]
fn checked_ir_identity_is_address_stable() {
    // Step 3 prerequisite. The shadow builder still keys finalized type facts
    // by AST pointer identity (TypeChecker::expr_key), which is valid only for
    // one in-memory AST instance. The Checked IR it produces, however, must be
    // a deterministic function of program *structure*, never of memory
    // addresses. Parsing the same source twice allocates two disjoint ASTs:
    // their checked-fact pointer keys are disjoint, yet the built IR - every
    // ExprId/BindingId/ValueId, layout, and flow fact - must be identical.
    let source = r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: (items: List<Int32>) -> List<Int32> = {
    val alias = items
    alias |> keep
}
"#;

    let (_, program_a) = parse_program(source).expect("source should parse");
    let (_, program_b) = parse_program(source).expect("source should parse");

    let mut checker_a = TypeChecker::new();
    checker_a
        .check_program(&program_a)
        .expect("source should type-check");
    let mut checker_b = TypeChecker::new();
    checker_b
        .check_program(&program_b)
        .expect("source should type-check");

    // Two independently parsed, distinct AST allocations for which the checkers
    // record the same number of facts. We deliberately do not assert raw-address
    // disjointness of the checker keys: pipe desugaring records freed
    // transient-clone Expr addresses, which an allocator may reuse across the two
    // checks, so key disjointness is not guaranteed. The real address-independence
    // property is the identical IR asserted below.
    assert!(!std::ptr::eq(&program_a, &program_b));
    let fact_count_a = checker_a.expr_types().len();
    let fact_count_b = checker_b.expr_types().len();
    assert!(fact_count_a > 0);
    assert_eq!(fact_count_a, fact_count_b);

    let ir_a = build_checked_ir(&program_a, &checker_a).expect("checked IR should build");
    let ir_b = build_checked_ir(&program_b, &checker_b).expect("checked IR should build");

    // ...yet the structural IR is identical: no address leaks into identity.
    assert_eq!(ir_a, ir_b);

    // Identity spaces are densely and deterministically assigned: ExprIds and
    // BindingIds each cover [0, N) exactly once across the whole program.
    assert_dense_identity_space(&ir_a);
}

fn assert_dense_identity_space(ir: &CheckedProgramIr) {
    let mut expr_ids = ir
        .functions
        .iter()
        .flat_map(|function| function.typed_exprs.iter().map(|expr| expr.id.0))
        .collect::<Vec<_>>();
    expr_ids.sort_unstable();
    assert_eq!(
        expr_ids,
        (0..expr_ids.len() as u32).collect::<Vec<_>>(),
        "ExprIds must densely cover [0, N) with no gaps or duplicates"
    );

    let mut binding_ids = ir
        .functions
        .iter()
        .flat_map(|function| function.bindings.iter().map(|binding| binding.id.0))
        .collect::<Vec<_>>();
    binding_ids.sort_unstable();
    assert_eq!(
        binding_ids,
        (0..binding_ids.len() as u32).collect::<Vec<_>>(),
        "BindingIds must densely cover [0, N) with no gaps or duplicates"
    );
}
