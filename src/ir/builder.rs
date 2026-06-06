//! Read-only Checked IR builder.
//!
//! This builder intentionally shadows the existing AST-driven codegen pipeline.
//! It does not re-run inference, does not inspect or mutate affine checker
//! state, and is not yet the codegen source of truth.

use std::collections::HashMap;

use crate::ast::*;
use crate::type_checker::{CheckedFunctionSignature, TypeChecker};

use super::layout::LayoutTable;
use super::{
    ApplyFlavor, ApplyIr, ExprId, FinalType, FlowSummary, TypedExpr, TypedExprKind, UseEvent,
    UseKind, ValueId, ValueRepr,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CheckedProgramIr {
    pub functions: Vec<CheckedFunctionIr>,
    pub layout_table: LayoutTable,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckedParamIr {
    pub name: String,
    pub final_type: FinalType,
    pub repr: ValueRepr,
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
    UnfinalizedType(String),
}

impl From<super::IrError> for IrBuildError {
    fn from(value: super::IrError) -> Self {
        match value {
            super::IrError::UnfinalizedType(ty) => IrBuildError::UnfinalizedType(ty),
            super::IrError::AffineDoubleUse(value) => {
                IrBuildError::UnfinalizedType(format!("unexpected affine event for {:?}", value))
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
            self.collect_function_ir_from_decl(decl, &mut functions)?;
        }

        Ok(CheckedProgramIr {
            functions,
            layout_table: self.layout_table,
        })
    }

    fn collect_function_ir_from_decl(
        &mut self,
        decl: &TopDecl,
        functions: &mut Vec<CheckedFunctionIr>,
    ) -> Result<(), IrBuildError> {
        match decl {
            TopDecl::Function(func) => functions.push(self.build_function_ir(func)?),
            TopDecl::Export(export) => {
                self.collect_function_ir_from_decl(export.item.as_ref(), functions)?
            }
            TopDecl::Impl(_) | TopDecl::Record(_) | TopDecl::Context(_) | TopDecl::Binding(_) => {}
        }
        Ok(())
    }

    fn build_function_ir(&mut self, func: &FunDecl) -> Result<CheckedFunctionIr, IrBuildError> {
        let signature = self
            .checker
            .checked_function_signature(&func.name)
            .ok_or_else(|| IrBuildError::MissingCheckedReturn(func.name.clone()))?;
        self.build_function_ir_from_signature(func, signature)
    }

    fn build_function_ir_from_signature(
        &mut self,
        func: &FunDecl,
        signature: CheckedFunctionSignature,
    ) -> Result<CheckedFunctionIr, IrBuildError> {
        let return_type = FinalType::new(signature.return_type)?;
        let return_repr = self.layout_table.value_repr_for_type(&return_type);

        let params = signature
            .params
            .iter()
            .map(|(name, ty)| self.build_param_ir(name, ty))
            .collect::<Result<Vec<_>, _>>()?;

        let mut apply_sites = Vec::new();
        let typed_exprs = self.collect_typed_exprs_from_block(&func.body, &mut apply_sites)?;

        let monomorphic = return_type.is_monomorphic()
            && params.iter().all(|param| param.final_type.is_monomorphic());

        Ok(CheckedFunctionIr {
            name: func.name.clone(),
            params,
            return_type,
            return_repr,
            apply_sites,
            typed_exprs,
            monomorphic,
        })
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
    ) -> Result<Vec<TypedExpr>, IrBuildError> {
        let mut exprs = Vec::new();
        self.push_typed_exprs_from_block(block, &mut exprs, sites)?;
        Ok(exprs)
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
            Some(ValueRepr::Unit | ValueRepr::Scalar(_)) => Ok(UseKind::ReadCopy),
            Some(ValueRepr::Ref(_) | ValueRepr::Closure { .. }) => Ok(UseKind::Move),
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
