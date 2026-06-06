//! Read-only Checked IR builder.
//!
//! This builder intentionally shadows the existing AST-driven codegen pipeline.
//! It does not re-run inference, does not inspect or mutate affine checker
//! state, and is not yet the codegen source of truth.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::type_checker::{CheckedFunctionSignature, TypeChecker};

use super::layout::{LayoutId, LayoutKind, LayoutTable};
use super::{
    ApplyFlavor, ApplyIr, ExprId, FinalType, FlowSummary, HostAbi, InternalOnlyReason, TypedExpr,
    TypedExprKind, UseEvent, UseKind, ValueId, ValueRepr,
};

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
    pub final_type: FinalType,
    pub repr: ValueRepr,
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

pub fn build_checked_ir(
    program: &Program,
    checker: &TypeChecker,
) -> Result<CheckedProgramIr, IrBuildError> {
    CheckedIrBuilder::new(checker).build(program)
}

struct CheckedIrBuilder<'a> {
    checker: &'a TypeChecker,
    layout_table: LayoutTable,
    value_reprs: HashMap<ValueId, ValueRepr>,
    next_expr: u32,
    next_value: u32,
    next_apply: usize,
}

impl<'a> CheckedIrBuilder<'a> {
    fn new(checker: &'a TypeChecker) -> Self {
        Self {
            checker,
            layout_table: LayoutTable::new(),
            value_reprs: HashMap::new(),
            next_expr: 0,
            next_value: 0,
            next_apply: 0,
        }
    }

    fn build(mut self, program: &Program) -> Result<CheckedProgramIr, IrBuildError> {
        let mut functions = Vec::new();
        for decl in &program.declarations {
            self.collect_function_ir_from_decl(decl, &mut functions, false)?;
        }

        let program_ir = CheckedProgramIr {
            functions,
            layout_table: self.layout_table,
        };
        program_ir.validate_lowering_summaries()?;
        Ok(program_ir)
    }

    fn collect_function_ir_from_decl(
        &mut self,
        decl: &TopDecl,
        functions: &mut Vec<CheckedFunctionIr>,
        source_exported: bool,
    ) -> Result<(), IrBuildError> {
        match decl {
            TopDecl::Function(func) => {
                functions.push(self.build_function_ir(func, source_exported)?)
            }
            TopDecl::Export(export) => {
                self.collect_function_ir_from_decl(export.item.as_ref(), functions, true)?
            }
            TopDecl::Impl(_) | TopDecl::Record(_) | TopDecl::Context(_) | TopDecl::Binding(_) => {}
        }
        Ok(())
    }

    fn build_function_ir(
        &mut self,
        func: &FunDecl,
        source_exported: bool,
    ) -> Result<CheckedFunctionIr, IrBuildError> {
        let signature = self
            .checker
            .checked_function_signature(&func.name)
            .ok_or_else(|| IrBuildError::MissingCheckedReturn(func.name.clone()))?;
        self.build_function_ir_from_signature(func, signature, source_exported)
    }

    fn build_function_ir_from_signature(
        &mut self,
        func: &FunDecl,
        signature: CheckedFunctionSignature,
        source_exported: bool,
    ) -> Result<CheckedFunctionIr, IrBuildError> {
        let return_type = FinalType::new(signature.return_type.clone())?;
        let return_repr = self.layout_table.value_repr_for_type(&return_type);

        let params = signature
            .params
            .iter()
            .map(|(name, ty)| self.build_param_ir(name, ty))
            .collect::<Result<Vec<_>, _>>()?;

        let mut apply_sites = Vec::new();
        let (typed_exprs, body_result) =
            self.collect_typed_exprs_from_block(&func.body, &mut apply_sites)?;

        let monomorphic = return_type.is_monomorphic()
            && params.iter().all(|param| param.final_type.is_monomorphic());
        let lowering = build_lowering_summary(
            source_exported,
            &signature,
            &params,
            &return_type,
            return_repr,
            body_result,
            &typed_exprs,
            &self.layout_table,
            monomorphic,
        );

        let function = CheckedFunctionIr {
            name: func.name.clone(),
            params,
            return_type,
            return_repr,
            apply_sites,
            typed_exprs,
            monomorphic,
            lowering,
        };
        function.validate_shadow_invariants()?;
        function.validate_lowering_summary(&self.layout_table)?;
        Ok(function)
    }

    fn build_param_ir(
        &mut self,
        name: &str,
        ty: &crate::type_checker::TypedType,
    ) -> Result<CheckedParamIr, IrBuildError> {
        let final_type = FinalType::new(ty.clone())?;
        let repr = self.layout_table.value_repr_for_type(&final_type);

        Ok(CheckedParamIr {
            name: name.to_string(),
            final_type,
            repr,
        })
    }

    fn collect_typed_exprs_from_block(
        &mut self,
        block: &BlockExpr,
        sites: &mut Vec<NormalizedApplySite>,
    ) -> Result<(Vec<TypedExpr>, Option<ValueId>), IrBuildError> {
        let mut exprs = Vec::new();
        let body_result = self.push_typed_exprs_from_block(block, &mut exprs, sites)?;
        Ok((exprs, body_result))
    }

    fn push_typed_exprs_from_block(
        &mut self,
        block: &BlockExpr,
        exprs: &mut Vec<TypedExpr>,
        sites: &mut Vec<NormalizedApplySite>,
    ) -> Result<Option<ValueId>, IrBuildError> {
        let mut last_value = None;
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(binding) => {
                    last_value = self.push_typed_exprs_from_expr(&binding.value, exprs, sites)?;
                }
                Stmt::Assignment(assign) => {
                    last_value = self.push_typed_exprs_from_expr(&assign.value, exprs, sites)?;
                }
                Stmt::Expr(expr) => {
                    last_value = self.push_typed_exprs_from_expr(expr, exprs, sites)?;
                }
            }
        }
        if let Some(expr) = &block.expr {
            last_value = self.push_typed_exprs_from_expr(expr, exprs, sites)?;
        }
        Ok(last_value)
    }

    fn push_typed_exprs_from_expr(
        &mut self,
        expr: &Expr,
        exprs: &mut Vec<TypedExpr>,
        sites: &mut Vec<NormalizedApplySite>,
    ) -> Result<Option<ValueId>, IrBuildError> {
        let mut apply = None;

        match expr {
            Expr::Call(call) => {
                let mut args = Vec::new();
                for arg in &call.args {
                    let value = self
                        .push_typed_exprs_from_expr(arg, exprs, sites)?
                        .ok_or_else(|| missing_apply_value("tuple argument"))?;
                    args.push(value);
                }
                let callee = self
                    .push_typed_exprs_from_expr(&call.function, exprs, sites)?
                    .unwrap_or_else(|| self.next_value());
                apply = Some(PendingApply {
                    callee_hint: callee_hint(call.function.as_ref()),
                    apply: self.make_call_apply(call, callee, args),
                });
            }
            Expr::Pipe(pipe) => {
                let object = self.push_typed_exprs_from_expr(&pipe.expr, exprs, sites)?;
                match &pipe.target {
                    PipeTarget::Expr(target) => {
                        let object = object.ok_or_else(|| missing_apply_value("pipe object"))?;
                        let callee = self
                            .push_typed_exprs_from_expr(target, exprs, sites)?
                            .unwrap_or_else(|| self.next_value());
                        apply = Some(PendingApply {
                            callee_hint: callee_hint(target),
                            apply: self.make_pipe_apply(callee, object),
                        });
                    }
                    PipeTarget::Ident(name) => {
                        if self.checker.checked_function_return_type(name).is_some() {
                            let object =
                                object.ok_or_else(|| missing_apply_value("pipe object"))?;
                            let callee = self.next_value();
                            apply = Some(PendingApply {
                                callee_hint: Some(name.clone()),
                                apply: self.make_pipe_apply(callee, object),
                            });
                        }
                    }
                }
            }
            Expr::RecordLit(record) => {
                for field in &record.fields {
                    self.push_typed_exprs_from_field_init(field, exprs, sites)?;
                }
            }
            Expr::Clone(clone) => {
                self.push_typed_exprs_from_expr(&clone.base, exprs, sites)?;
                for field in &clone.updates.fields {
                    self.push_typed_exprs_from_field_init(field, exprs, sites)?;
                }
            }
            Expr::Freeze(inner)
            | Expr::Await(inner)
            | Expr::Spawn(inner)
            | Expr::Some(inner)
            | Expr::Ok(inner)
            | Expr::Err(inner)
            | Expr::FieldAccess(inner, _) => {
                self.push_typed_exprs_from_expr(inner, exprs, sites)?;
            }
            Expr::PrototypeClone(clone) => {
                for field in &clone.updates.fields {
                    self.push_typed_exprs_from_field_init(field, exprs, sites)?;
                }
            }
            Expr::Then(then) => {
                self.push_typed_exprs_from_expr(&then.condition, exprs, sites)?;
                self.push_typed_exprs_from_block(&then.then_block, exprs, sites)?;
                for (condition, block) in &then.else_ifs {
                    self.push_typed_exprs_from_expr(condition, exprs, sites)?;
                    self.push_typed_exprs_from_block(block, exprs, sites)?;
                }
                if let Some(block) = &then.else_block {
                    self.push_typed_exprs_from_block(block, exprs, sites)?;
                }
            }
            Expr::While(while_expr) => {
                self.push_typed_exprs_from_expr(&while_expr.condition, exprs, sites)?;
                self.push_typed_exprs_from_block(&while_expr.body, exprs, sites)?;
            }
            Expr::Match(match_expr) => {
                self.push_typed_exprs_from_expr(&match_expr.expr, exprs, sites)?;
                for arm in &match_expr.arms {
                    self.push_typed_exprs_from_block(&arm.body, exprs, sites)?;
                }
            }
            Expr::Binary(binary) => {
                self.push_typed_exprs_from_expr(&binary.left, exprs, sites)?;
                self.push_typed_exprs_from_expr(&binary.right, exprs, sites)?;
            }
            Expr::Unary(unary) => {
                self.push_typed_exprs_from_expr(&unary.expr, exprs, sites)?;
            }
            Expr::Cast(cast) => {
                self.push_typed_exprs_from_expr(&cast.expr, exprs, sites)?;
            }
            Expr::With(with) => {
                for binding in &with.bindings {
                    self.push_typed_exprs_from_field_init(binding, exprs, sites)?;
                }
                self.push_typed_exprs_from_block(&with.body, exprs, sites)?;
            }
            Expr::WithLifetime(with) => {
                self.push_typed_exprs_from_block(&with.body, exprs, sites)?;
            }
            Expr::Block(block) => {
                self.push_typed_exprs_from_block(block, exprs, sites)?;
            }
            Expr::ListLit(items) | Expr::ArrayLit(items) => {
                for item in items {
                    self.push_typed_exprs_from_expr(item, exprs, sites)?;
                }
            }
            Expr::RangeLit(range) => {
                self.push_typed_exprs_from_expr(&range.start, exprs, sites)?;
                self.push_typed_exprs_from_expr(&range.end, exprs, sites)?;
            }
            Expr::Lambda(lambda) => {
                self.push_typed_exprs_from_expr(&lambda.body, exprs, sites)?;
            }
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::Ident(_)
            | Expr::None => {}
        }

        if let Some(typed_expr) = self.build_typed_expr_skeleton(expr, apply, sites)? {
            let value = typed_expr.value;
            exprs.push(typed_expr);
            return Ok(value);
        }

        Ok(None)
    }

    fn push_typed_exprs_from_field_init(
        &mut self,
        field: &FieldInit,
        exprs: &mut Vec<TypedExpr>,
        sites: &mut Vec<NormalizedApplySite>,
    ) -> Result<(), IrBuildError> {
        match field {
            FieldInit::Field { value, .. } | FieldInit::Spread(value) => {
                self.push_typed_exprs_from_expr(value, exprs, sites)?;
                Ok(())
            }
        }
    }

    fn build_typed_expr_skeleton(
        &mut self,
        expr: &Expr,
        apply: Option<PendingApply>,
        sites: &mut Vec<NormalizedApplySite>,
    ) -> Result<Option<TypedExpr>, IrBuildError> {
        let ty = match self.checker.checked_expr_type(expr) {
            Some(ty) => ty,
            None => {
                if let Some(pending) = apply {
                    return Err(IrBuildError::MissingCheckedExprType(
                        pending.callee_hint.unwrap_or_else(|| "apply".to_string()),
                    ));
                }
                return Ok(None);
            }
        };

        let final_type = FinalType::new(ty)?;
        let repr = self.layout_table.value_repr_for_type(&final_type);
        let id = self.next_expr_id();
        let (value, kind, apply_args) = match apply {
            Some(pending) => {
                let source_index = self.next_apply_index();
                let apply = pending.apply;
                let args = apply.args.clone();
                sites.push(NormalizedApplySite {
                    source_index,
                    expr_id: id,
                    callee_hint: pending.callee_hint,
                    apply: apply.clone(),
                });
                (Some(apply.result), TypedExprKind::Apply(apply), args)
            }
            None => {
                let value = self.next_value();
                let kind = if is_literal_expr(expr) {
                    TypedExprKind::Literal
                } else {
                    TypedExprKind::Value(value)
                };
                (Some(value), kind, Vec::new())
            }
        };

        let mut flow = FlowSummary::new();
        for arg in apply_args {
            flow.record_use(UseEvent {
                value: arg,
                kind: self.use_kind_for_value(arg)?,
                at: id,
            });
        }
        if let Some(value) = value {
            flow.record_produced(value);
            self.value_reprs.insert(value, repr);
        }

        Ok(Some(TypedExpr {
            id,
            value,
            final_type,
            repr,
            flow,
            kind,
        }))
    }

    fn use_kind_for_value(&self, value: ValueId) -> Result<UseKind, IrBuildError> {
        match self.value_reprs.get(&value).copied() {
            Some(repr) => Ok(use_kind_for_repr(repr)),
            None => Err(IrBuildError::MissingValueRepr(value)),
        }
    }

    fn make_call_apply(&mut self, call: &CallExpr, callee: ValueId, args: Vec<ValueId>) -> ApplyIr {
        let flavor = match call.function.as_ref() {
            Expr::Lambda(_) => ApplyFlavor::ImmediateLambda,
            Expr::FieldAccess(_, _) => ApplyFlavor::MethodResolution,
            Expr::Ident(_) if call.args.is_empty() => ApplyFlavor::UnitCall,
            Expr::Ident(_) => ApplyFlavor::TupleCall,
            _ => ApplyFlavor::FunctionValue,
        };
        let result = self.next_value();

        ApplyIr {
            flavor,
            callee,
            args,
            result,
        }
    }

    fn make_pipe_apply(&mut self, callee: ValueId, object: ValueId) -> ApplyIr {
        let result = self.next_value();

        ApplyIr {
            flavor: ApplyFlavor::Pipe,
            callee,
            args: vec![object],
            result,
        }
    }

    fn next_expr_id(&mut self) -> ExprId {
        let id = ExprId(self.next_expr);
        self.next_expr += 1;
        id
    }

    fn next_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value);
        self.next_value += 1;
        id
    }

    fn next_apply_index(&mut self) -> usize {
        let index = self.next_apply;
        self.next_apply += 1;
        index
    }
}

fn missing_apply_value(value: &str) -> IrBuildError {
    IrBuildError::MissingApplyValue(value.to_string())
}

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

fn shadow_invariant_violation(message: String) -> IrBuildError {
    IrBuildError::ShadowInvariantViolation(message)
}

fn lowering_invariant_violation(message: String) -> IrBuildError {
    IrBuildError::LoweringInvariantViolation(message)
}

fn is_literal_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::ListLit(_)
            | Expr::ArrayLit(_)
            | Expr::None
    )
}

fn callee_hint(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.clone()),
        Expr::FieldAccess(_, field) => Some(field.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(main.apply_sites[0].source_index, 0);
        assert_eq!(main.apply_sites[1].source_index, 1);
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
            args: vec![ValueId(1)],
            result: ValueId(2),
        };
        let function = CheckedFunctionIr {
            name: "main".to_string(),
            params: Vec::new(),
            return_type: FinalType::new(TypedType::Int32).unwrap(),
            return_repr: ValueRepr::Scalar(ScalarRepr::I32),
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
            .find(|expr| matches!(expr.kind, TypedExprKind::Value(_)))
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
}
