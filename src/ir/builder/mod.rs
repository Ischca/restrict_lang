//! Read-only Checked IR builder.
//!
//! This builder intentionally shadows the existing AST-driven codegen pipeline.
//! It does not re-run inference, does not inspect or mutate affine checker
//! state, and is not yet the codegen source of truth.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::type_checker::{CheckedFunctionSignature, TypeChecker, TypedType};

use super::layout::{LayoutId, LayoutKind, LayoutTable};
use super::{
    ApplyFlavor, ApplyIr, BindingId, CalleeProvenance, ExprId, FinalType, FlowSummary,
    FunctionCalleeIr, HostAbi, InternalOnlyReason, TypedExpr, TypedExprKind, UseEvent, UseKind,
    ValueId, ValueRepr,
};

mod construct;
pub use construct::build_checked_ir;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub struct CheckedProgramIr {
    pub functions: Vec<CheckedFunctionIr>,
    pub layout_table: LayoutTable,
}

impl CheckedProgramIr {
    pub fn validate_lowering_summaries(&self) -> Result<(), IrBuildError> {
        for function in &self.functions {
            function.validate_lowering_summary(&self.layout_table)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckedFunctionIr {
    pub name: String,
    pub params: Vec<CheckedParamIr>,
    pub return_type: FinalType,
    pub return_repr: ValueRepr,
    pub bindings: Vec<CheckedBindingIr>,
    pub apply_sites: Vec<NormalizedApplySite>,
    pub typed_exprs: Vec<TypedExpr>,
    pub monomorphic: bool,
    pub lowering: CheckedFunctionLoweringSummary,
}

impl CheckedFunctionIr {
    pub fn validate_shadow_invariants(&self) -> Result<(), IrBuildError> {
        let mut exprs_by_id = HashMap::new();
        let mut expr_positions = HashMap::new();
        let mut value_producers = HashMap::new();

        for (index, expr) in self.typed_exprs.iter().enumerate() {
            if exprs_by_id.insert(expr.id, expr).is_some() {
                return Err(shadow_invariant_violation(format!(
                    "duplicate typed expression id {:?} in {}",
                    expr.id, self.name
                )));
            }
            expr_positions.insert(expr.id, index);
            for value in expr.flow.produced() {
                if value_producers.insert(*value, (index, expr.repr)).is_some() {
                    return Err(shadow_invariant_violation(format!(
                        "value {:?} in {} is produced more than once",
                        value, self.name
                    )));
                }
            }
        }

        let mut bindings_by_id = HashMap::new();
        for binding in &self.bindings {
            if bindings_by_id.insert(binding.id, binding).is_some() {
                return Err(shadow_invariant_violation(format!(
                    "duplicate binding id {:?} in {}",
                    binding.id, self.name
                )));
            }
            if let Some(value) = binding.value {
                if !value_producers.contains_key(&value) {
                    return Err(shadow_invariant_violation(format!(
                        "binding {:?} in {} points at unproduced value {:?}",
                        binding.id, self.name, value
                    )));
                }
            }
        }

        for (index, param) in self.params.iter().enumerate() {
            let binding = bindings_by_id.get(&param.binding).ok_or_else(|| {
                shadow_invariant_violation(format!(
                    "parameter '{}' in {} points at missing binding {:?}",
                    param.name, self.name, param.binding
                ))
            })?;
            if binding.name != param.name
                || binding.source != (CheckedBindingSource::Param { index })
                || binding.value.is_some()
                || binding.final_type != param.final_type
                || binding.repr != param.repr
            {
                return Err(shadow_invariant_violation(format!(
                    "parameter '{}' in {} has stale binding provenance",
                    param.name, self.name
                )));
            }
        }

        // Every binding-read expression must resolve to a binding that this
        // function actually declares. Builder-local BindingIds are not yet stable
        // symbol identities, so the shadow graph at least guarantees that no read
        // dangles past the bindings it was resolved against.
        for expr in &self.typed_exprs {
            if let TypedExprKind::Binding(binding) = &expr.kind {
                if !bindings_by_id.contains_key(binding) {
                    return Err(shadow_invariant_violation(format!(
                        "binding expression {:?} in {} references missing binding {:?}",
                        expr.id, self.name, binding
                    )));
                }
            }
        }

        let apply_expr_count = self
            .typed_exprs
            .iter()
            .filter(|expr| matches!(expr.kind, TypedExprKind::Apply(_)))
            .count();
        if self.apply_sites.len() != apply_expr_count {
            return Err(shadow_invariant_violation(format!(
                "{} has {} apply sites but {} typed apply expressions",
                self.name,
                self.apply_sites.len(),
                apply_expr_count
            )));
        }

        let mut apply_expr_ids = HashSet::new();
        let mut moved_values = HashSet::new();
        for (expected_source_index, site) in self.apply_sites.iter().enumerate() {
            if site.source_index != expected_source_index {
                return Err(shadow_invariant_violation(format!(
                    "apply site in {} has source index {}, expected {}",
                    self.name, site.source_index, expected_source_index
                )));
            }
            let expr = exprs_by_id.get(&site.expr_id).ok_or_else(|| {
                shadow_invariant_violation(format!(
                    "apply site {} in {} points at missing expr {:?}",
                    site.source_index, self.name, site.expr_id
                ))
            })?;
            let TypedExprKind::Apply(apply) = &expr.kind else {
                return Err(shadow_invariant_violation(format!(
                    "apply site {} in {} points at non-apply expr {:?}",
                    site.source_index, self.name, site.expr_id
                )));
            };

            if apply != &site.apply {
                return Err(shadow_invariant_violation(format!(
                    "apply site {} in {} does not match typed expr {:?}",
                    site.source_index, self.name, site.expr_id
                )));
            }
            validate_apply_callee_provenance(&site.apply, site.callee_hint.as_deref(), &self.name)?;
            if expr.value != Some(site.apply.result) {
                return Err(shadow_invariant_violation(format!(
                    "apply site {} in {} result does not match typed expr value",
                    site.source_index, self.name
                )));
            }
            if expr.flow.produced() != [site.apply.result] {
                return Err(shadow_invariant_violation(format!(
                    "apply site {} in {} has invalid produced flow",
                    site.source_index, self.name
                )));
            }
            let expr_index = *expr_positions.get(&expr.id).ok_or_else(|| {
                shadow_invariant_violation(format!(
                    "apply site {} in {} has no expression position",
                    site.source_index, self.name
                ))
            })?;
            let used_values = expr
                .flow
                .uses()
                .iter()
                .map(|event| event.value)
                .collect::<Vec<_>>();
            if used_values != site.apply.args {
                return Err(shadow_invariant_violation(format!(
                    "apply site {} in {} has invalid argument use flow",
                    site.source_index, self.name
                )));
            }
            for event in expr.flow.uses() {
                if event.at != expr.id {
                    return Err(shadow_invariant_violation(format!(
                        "apply site {} in {} has use events at the wrong expr",
                        site.source_index, self.name
                    )));
                }
                let (producer_index, producer_repr) =
                    value_producers.get(&event.value).ok_or_else(|| {
                        shadow_invariant_violation(format!(
                            "apply site {} in {} uses unproduced value {:?}",
                            site.source_index, self.name, event.value
                        ))
                    })?;
                if *producer_index >= expr_index {
                    return Err(shadow_invariant_violation(format!(
                        "apply site {} in {} uses value {:?} before it is produced",
                        site.source_index, self.name, event.value
                    )));
                }
                let expected_kind = use_kind_for_repr(*producer_repr);
                if event.kind != expected_kind {
                    return Err(shadow_invariant_violation(format!(
                        "apply site {} in {} uses value {:?} as {:?}, expected {:?}",
                        site.source_index, self.name, event.value, event.kind, expected_kind
                    )));
                }
                if matches!(event.kind, UseKind::Move | UseKind::Drop)
                    && !moved_values.insert(event.value)
                {
                    return Err(shadow_invariant_violation(format!(
                        "value {:?} in {} is moved more than once",
                        event.value, self.name
                    )));
                }
            }

            apply_expr_ids.insert(site.expr_id);
        }

        for expr in &self.typed_exprs {
            if matches!(expr.kind, TypedExprKind::Apply(_)) && !apply_expr_ids.contains(&expr.id) {
                return Err(shadow_invariant_violation(format!(
                    "typed apply expr {:?} in {} has no apply site",
                    expr.id, self.name
                )));
            }
        }

        Ok(())
    }

    pub fn validate_lowering_summary(
        &self,
        layout_table: &LayoutTable,
    ) -> Result<(), IrBuildError> {
        self.validate_shadow_invariants()?;

        if self.lowering.param_host_abis.len() != self.params.len() {
            return Err(lowering_invariant_violation(format!(
                "{} has {} parameter host ABIs for {} parameters",
                self.name,
                self.lowering.param_host_abis.len(),
                self.params.len()
            )));
        }

        for (index, param) in self.params.iter().enumerate() {
            let expected = param.final_type.host_abi();
            if self.lowering.param_host_abis[index] != expected {
                return Err(lowering_invariant_violation(format!(
                    "{} parameter '{}' host ABI summary is stale",
                    self.name, param.name
                )));
            }
        }

        if self.lowering.return_host_abi != self.return_type.host_abi() {
            return Err(lowering_invariant_violation(format!(
                "{} return host ABI summary is stale",
                self.name
            )));
        }

        let required_layouts = collect_required_layouts(
            &self.params,
            self.return_repr,
            &self.typed_exprs,
            layout_table,
        );
        if self.lowering.required_layouts != required_layouts {
            return Err(lowering_invariant_violation(format!(
                "{} required layout summary is stale",
                self.name
            )));
        }

        for layout in &self.lowering.required_layouts {
            if layout_table.get(*layout).is_none() {
                return Err(lowering_invariant_violation(format!(
                    "{} references missing layout {:?}",
                    self.name, layout
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckedParamIr {
    pub name: String,
    pub binding: BindingId,
    pub final_type: FinalType,
    pub repr: ValueRepr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckedBindingIr {
    pub id: BindingId,
    pub name: String,
    pub mutable: bool,
    pub source: CheckedBindingSource,
    pub value: Option<ValueId>,
    pub final_type: FinalType,
    pub repr: ValueRepr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedBindingSource {
    Param { index: usize },
    Local,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFunctionLoweringSummary {
    pub source_exported: bool,
    pub declared_type_params: Vec<String>,
    pub temporal_constraints: Vec<CheckedTemporalConstraintIr>,
    pub param_host_abis: Vec<HostAbi>,
    pub return_host_abi: HostAbi,
    pub body_result: Option<ValueId>,
    pub required_layouts: Vec<LayoutId>,
    pub readiness: LoweringReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedTemporalConstraintIr {
    pub inner: String,
    pub outer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringReadiness {
    pub v001_host_abi_eligible: bool,
    pub internal_layout_ready: bool,
    pub host_abi_blockers: Vec<HostAbiBlocker>,
    pub internal_lowering_blockers: Vec<InternalLoweringBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAbiBlocker {
    DeclaredTypeParam(String),
    TemporalConstraint(CheckedTemporalConstraintIr),
    NonMonomorphicSignature,
    ParamInternalOnly {
        name: String,
        reason: InternalOnlyReason,
    },
    ReturnInternalOnly {
        reason: InternalOnlyReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternalLoweringBlocker {
    MissingBodyResult,
    MissingBodyProducer(ValueId),
    BodyResultTypeMismatch {
        value: ValueId,
        expected: String,
        actual: String,
    },
    MissingRequiredLayout(LayoutId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedApplySite {
    pub source_index: usize,
    pub expr_id: ExprId,
    pub callee_hint: Option<String>,
    pub apply: ApplyIr,
}

#[derive(Debug, Clone, PartialEq)]
struct PendingApply {
    callee_hint: Option<String>,
    apply: ApplyIr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingScopeEntry {
    Known(BindingId),
    ShadowOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrBuildError {
    MissingCheckedReturn(String),
    MissingCheckedExprType(String),
    MissingApplyValue(String),
    MissingValueRepr(ValueId),
    ShadowInvariantViolation(String),
    LoweringInvariantViolation(String),
    UnfinalizedType(String),
}

impl From<super::IrError> for IrBuildError {
    fn from(value: super::IrError) -> Self {
        match value {
            super::IrError::UnfinalizedType(ty) => IrBuildError::UnfinalizedType(ty),
            super::IrError::AffineDoubleUse(value) => {
                shadow_invariant_violation(format!("unexpected affine double-use for {:?}", value))
            }
        }
    }
}

fn missing_apply_value(value: &str) -> IrBuildError {
    IrBuildError::MissingApplyValue(value.to_string())
}

// Gathers every finalized fact needed for one function's lowering summary in a
// single pass; the parameters are cohesive inputs rather than separable concerns,
// so they stay positional instead of being grouped into a one-off struct.
#[allow(clippy::too_many_arguments)]
fn build_lowering_summary(
    source_exported: bool,
    signature: &CheckedFunctionSignature,
    params: &[CheckedParamIr],
    return_type: &FinalType,
    return_repr: ValueRepr,
    body_result: Option<ValueId>,
    typed_exprs: &[TypedExpr],
    layout_table: &LayoutTable,
    monomorphic: bool,
) -> CheckedFunctionLoweringSummary {
    let declared_type_params = signature
        .type_params
        .iter()
        .map(format_type_param_name)
        .collect::<Vec<_>>();
    let temporal_constraints = signature
        .temporal_constraints
        .iter()
        .map(|constraint| CheckedTemporalConstraintIr {
            inner: constraint.inner.clone(),
            outer: constraint.outer.clone(),
        })
        .collect::<Vec<_>>();
    let param_host_abis = params
        .iter()
        .map(|param| param.final_type.host_abi())
        .collect::<Vec<_>>();
    let return_host_abi = return_type.host_abi();
    let required_layouts = collect_required_layouts(params, return_repr, typed_exprs, layout_table);

    let mut host_abi_blockers = Vec::new();
    for type_param in &declared_type_params {
        host_abi_blockers.push(HostAbiBlocker::DeclaredTypeParam(type_param.clone()));
    }
    for constraint in &temporal_constraints {
        host_abi_blockers.push(HostAbiBlocker::TemporalConstraint(constraint.clone()));
    }
    if !monomorphic {
        host_abi_blockers.push(HostAbiBlocker::NonMonomorphicSignature);
    }
    for (param, abi) in params.iter().zip(&param_host_abis) {
        if let HostAbi::InternalOnly(reason) = abi {
            host_abi_blockers.push(HostAbiBlocker::ParamInternalOnly {
                name: param.name.clone(),
                reason: reason.clone(),
            });
        }
    }
    if let HostAbi::InternalOnly(reason) = &return_host_abi {
        host_abi_blockers.push(HostAbiBlocker::ReturnInternalOnly {
            reason: reason.clone(),
        });
    }

    let mut internal_lowering_blockers = Vec::new();
    if !matches!(
        return_type.as_typed_type(),
        crate::type_checker::TypedType::Unit
    ) {
        match body_result {
            Some(value) => match producer_for_value(typed_exprs, value) {
                Some(producer) if producer.final_type != *return_type => {
                    internal_lowering_blockers.push(
                        InternalLoweringBlocker::BodyResultTypeMismatch {
                            value,
                            expected: crate::type_checker::format_typed_type(
                                return_type.as_typed_type(),
                            ),
                            actual: crate::type_checker::format_typed_type(
                                producer.final_type.as_typed_type(),
                            ),
                        },
                    );
                }
                Some(_) => {}
                None => internal_lowering_blockers
                    .push(InternalLoweringBlocker::MissingBodyProducer(value)),
            },
            None => internal_lowering_blockers.push(InternalLoweringBlocker::MissingBodyResult),
        }
    }
    for layout in &required_layouts {
        if layout_table.get(*layout).is_none() {
            internal_lowering_blockers
                .push(InternalLoweringBlocker::MissingRequiredLayout(*layout));
        }
    }

    CheckedFunctionLoweringSummary {
        source_exported,
        declared_type_params,
        temporal_constraints,
        param_host_abis,
        return_host_abi,
        body_result,
        required_layouts,
        readiness: LoweringReadiness {
            v001_host_abi_eligible: host_abi_blockers.is_empty()
                && monomorphic
                && params
                    .iter()
                    .all(|param| param.final_type.host_abi().is_v001_exportable())
                && return_type.host_abi().is_v001_exportable(),
            internal_layout_ready: internal_lowering_blockers.is_empty(),
            host_abi_blockers,
            internal_lowering_blockers,
        },
    }
}

fn format_type_param_name(param: &TypeParam) -> String {
    if param.is_temporal {
        format!("~{}", param.name)
    } else {
        param.name.clone()
    }
}

fn contains_record_layout_type_param(ty: &TypedType) -> bool {
    match ty {
        TypedType::TypeParam(_) | TypedType::InferVar(_) | TypedType::Projection { .. } => true,
        TypedType::Record { type_args, .. } => {
            type_args.iter().any(contains_record_layout_type_param)
        }
        TypedType::Function {
            params,
            return_type,
        } => {
            params.iter().any(contains_record_layout_type_param)
                || contains_record_layout_type_param(return_type)
        }
        TypedType::Option(inner)
        | TypedType::List(inner)
        | TypedType::Array(inner, _)
        | TypedType::Temporal {
            base_type: inner, ..
        } => contains_record_layout_type_param(inner),
        TypedType::Result(ok, err) => {
            contains_record_layout_type_param(ok) || contains_record_layout_type_param(err)
        }
        TypedType::Int32
        | TypedType::Int64
        | TypedType::Float64
        | TypedType::Boolean
        | TypedType::String
        | TypedType::Char
        | TypedType::Unit => false,
    }
}

fn producer_for_value(typed_exprs: &[TypedExpr], value: ValueId) -> Option<&TypedExpr> {
    typed_exprs
        .iter()
        .find(|expr| expr.flow.produced().contains(&value))
}

fn collect_required_layouts(
    params: &[CheckedParamIr],
    return_repr: ValueRepr,
    typed_exprs: &[TypedExpr],
    layout_table: &LayoutTable,
) -> Vec<LayoutId> {
    let mut layouts = HashSet::new();
    for param in params {
        collect_layouts_from_repr(param.repr, layout_table, &mut layouts);
    }
    collect_layouts_from_repr(return_repr, layout_table, &mut layouts);
    for expr in typed_exprs {
        collect_layouts_from_repr(expr.repr, layout_table, &mut layouts);
    }

    let mut layouts = layouts.into_iter().collect::<Vec<_>>();
    layouts.sort_by_key(|layout| layout.0);
    layouts
}

fn collect_layouts_from_repr(
    repr: ValueRepr,
    layout_table: &LayoutTable,
    layouts: &mut HashSet<LayoutId>,
) {
    match repr {
        ValueRepr::Ref(layout) | ValueRepr::Closure { layout, .. } => {
            collect_layout(layout, layout_table, layouts);
        }
        ValueRepr::Unit | ValueRepr::Scalar(_) => {}
    }
}

fn collect_layout(layout: LayoutId, layout_table: &LayoutTable, layouts: &mut HashSet<LayoutId>) {
    if !layouts.insert(layout) {
        return;
    }

    let Some(descriptor) = layout_table.get(layout) else {
        return;
    };

    match &descriptor.kind {
        LayoutKind::String(_) | LayoutKind::Opaque(_) => {}
        LayoutKind::List(list) => {
            collect_layouts_from_repr(list.element.repr, layout_table, layouts);
        }
        LayoutKind::Array(array) => {
            collect_layouts_from_repr(array.element.repr, layout_table, layouts);
        }
        LayoutKind::Range(range) => {
            collect_layouts_from_repr(range.endpoint.repr, layout_table, layouts);
        }
        LayoutKind::Record(record) => {
            for field in &record.fields {
                collect_layouts_from_repr(field.element.repr, layout_table, layouts);
            }
        }
        LayoutKind::Sum(sum) => {
            for variant in &sum.variants {
                if let Some(payload) = &variant.payload {
                    collect_layouts_from_repr(payload.repr, layout_table, layouts);
                }
            }
        }
        LayoutKind::Closure(closure) => {
            for param in &closure.params {
                collect_layouts_from_repr(param.repr, layout_table, layouts);
            }
            collect_layouts_from_repr(closure.result.repr, layout_table, layouts);
            for capture in &closure.captures {
                collect_layouts_from_repr(capture.repr, layout_table, layouts);
            }
        }
    }
}

fn use_kind_for_repr(repr: ValueRepr) -> UseKind {
    match repr {
        ValueRepr::Unit | ValueRepr::Scalar(_) => UseKind::ReadCopy,
        ValueRepr::Ref(_) | ValueRepr::Closure { .. } => UseKind::Move,
    }
}

fn validate_apply_callee_provenance(
    apply: &ApplyIr,
    callee_hint: Option<&str>,
    function_name: &str,
) -> Result<(), IrBuildError> {
    let CalleeProvenance::TopLevelFunction(callee) = &apply.callee_provenance else {
        return Ok(());
    };
    if callee.name.is_empty() {
        return Err(shadow_invariant_violation(format!(
            "apply in {} has empty top-level callee name",
            function_name
        )));
    }
    if let Some(hint) = callee_hint {
        if hint != callee.name {
            return Err(shadow_invariant_violation(format!(
                "apply in {} has callee hint '{}' but top-level callee '{}'",
                function_name, hint, callee.name
            )));
        }
    }
    let expected_monomorphic =
        callee.return_type.is_monomorphic() && callee.params.iter().all(FinalType::is_monomorphic);
    if callee.monomorphic != expected_monomorphic {
        return Err(shadow_invariant_violation(format!(
            "apply in {} has stale monomorphic callee provenance for '{}'",
            function_name, callee.name
        )));
    }
    Ok(())
}

fn shadow_invariant_violation(message: String) -> IrBuildError {
    IrBuildError::ShadowInvariantViolation(message)
}

fn lowering_invariant_violation(message: String) -> IrBuildError {
    IrBuildError::LoweringInvariantViolation(message)
}

fn is_literal_expr(expr: &Expr) -> bool {
    matches!(
        &expr.kind,
        ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::Unit
            | ExprKind::ListLit(_)
            | ExprKind::ArrayLit(_)
            | ExprKind::None
    )
}

fn callee_hint(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Ident(name) => Some(name.clone()),
        ExprKind::FieldAccess(_, field) => Some(field.clone()),
        _ => None,
    }
}

fn pattern_bound_names(pattern: &Pattern) -> Vec<String> {
    let mut names = Vec::new();
    collect_pattern_bound_names(pattern, &mut names);
    names
}

fn collect_pattern_bound_names(pattern: &Pattern, names: &mut Vec<String>) {
    match pattern {
        Pattern::Ident(name) if name != "_" => names.push(name.clone()),
        Pattern::Record(_, fields) => {
            for (_, field_pattern) in fields {
                collect_pattern_bound_names(field_pattern, names);
            }
        }
        Pattern::RecordDestruct { fields, rest, .. } => {
            for (_, field_pattern) in fields {
                collect_pattern_bound_names(field_pattern, names);
            }
            if let Some(rest) = rest {
                if rest != "_" {
                    names.push(rest.clone());
                }
            }
        }
        Pattern::Some(inner) | Pattern::Ok(inner) | Pattern::Err(inner) => {
            collect_pattern_bound_names(inner, names);
        }
        Pattern::ListCons(head, tail) => {
            collect_pattern_bound_names(head, names);
            collect_pattern_bound_names(tail, names);
        }
        Pattern::ListExact(patterns) => {
            for pattern in patterns {
                collect_pattern_bound_names(pattern, names);
            }
        }
        Pattern::Wildcard
        | Pattern::Literal(_)
        | Pattern::None
        | Pattern::EmptyList
        | Pattern::Ident(_) => {}
    }
}
