//! Checked IR construction: the AST walk that builds the read-only shadow IR.

use super::*;

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
    binding_scopes: Vec<HashMap<String, BindingScopeEntry>>,
    next_expr: u32,
    next_value: u32,
    next_binding: u32,
    next_apply: usize,
}

impl<'a> CheckedIrBuilder<'a> {
    fn new(checker: &'a TypeChecker) -> Self {
        Self {
            checker,
            layout_table: LayoutTable::new(),
            value_reprs: HashMap::new(),
            binding_scopes: Vec::new(),
            next_expr: 0,
            next_value: 0,
            next_binding: 0,
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

    fn value_repr_for_type(&mut self, final_type: &FinalType) -> ValueRepr {
        let checker = self.checker;
        self.layout_table
            .value_repr_for_type_with_record_fields(final_type, &|name, type_args| {
                if type_args.iter().any(contains_record_layout_type_param) {
                    return None;
                }
                let record_type = TypedType::Record {
                    name: name.to_string(),
                    type_args: type_args.to_vec(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                };
                checker.checked_record_fields_for_type(&record_type)
            })
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
        let return_repr = self.value_repr_for_type(&return_type);
        self.start_function_binding_scope();
        let mut bindings = Vec::new();

        let params = signature
            .params
            .iter()
            .enumerate()
            .map(|(index, (name, ty))| self.build_param_ir(index, name, ty, &mut bindings))
            .collect::<Result<Vec<_>, _>>()?;

        let mut apply_sites = Vec::new();
        let (typed_exprs, body_result) =
            self.collect_typed_exprs_from_block(&func.body, &mut apply_sites, &mut bindings)?;
        self.finish_function_binding_scope();

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
            bindings,
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
        index: usize,
        name: &str,
        ty: &crate::type_checker::TypedType,
        bindings: &mut Vec<CheckedBindingIr>,
    ) -> Result<CheckedParamIr, IrBuildError> {
        let final_type = FinalType::new(ty.clone())?;
        let repr = self.value_repr_for_type(&final_type);
        let binding = self.next_binding_id();
        self.register_binding_name(name, binding);
        bindings.push(CheckedBindingIr {
            id: binding,
            name: name.to_string(),
            mutable: false,
            source: CheckedBindingSource::Param { index },
            value: None,
            final_type: final_type.clone(),
            repr,
        });

        Ok(CheckedParamIr {
            name: name.to_string(),
            binding,
            final_type,
            repr,
        })
    }

    fn collect_typed_exprs_from_block(
        &mut self,
        block: &BlockExpr,
        sites: &mut Vec<NormalizedApplySite>,
        bindings: &mut Vec<CheckedBindingIr>,
    ) -> Result<(Vec<TypedExpr>, Option<ValueId>), IrBuildError> {
        let mut exprs = Vec::new();
        let body_result = self.push_typed_exprs_from_block(block, &mut exprs, sites, bindings)?;
        Ok((exprs, body_result))
    }

    fn push_typed_exprs_from_block(
        &mut self,
        block: &BlockExpr,
        exprs: &mut Vec<TypedExpr>,
        sites: &mut Vec<NormalizedApplySite>,
        bindings: &mut Vec<CheckedBindingIr>,
    ) -> Result<Option<ValueId>, IrBuildError> {
        self.push_binding_scope();
        let mut last_value = None;
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(binding) => {
                    last_value =
                        self.push_typed_exprs_from_expr(&binding.value, exprs, sites, bindings)?;
                    self.register_local_binding(binding, last_value, exprs, bindings)?;
                }
                Stmt::Assignment(assign) => {
                    last_value =
                        self.push_typed_exprs_from_expr(&assign.value, exprs, sites, bindings)?;
                }
                Stmt::Expr(expr) => {
                    last_value = self.push_typed_exprs_from_expr(expr, exprs, sites, bindings)?;
                }
            }
        }
        if let Some(expr) = &block.expr {
            last_value = self.push_typed_exprs_from_expr(expr, exprs, sites, bindings)?;
        }
        self.pop_binding_scope();
        Ok(last_value)
    }

    fn push_typed_exprs_from_expr(
        &mut self,
        expr: &Expr,
        exprs: &mut Vec<TypedExpr>,
        sites: &mut Vec<NormalizedApplySite>,
        bindings: &mut Vec<CheckedBindingIr>,
    ) -> Result<Option<ValueId>, IrBuildError> {
        let mut apply = None;

        match &expr.kind {
            ExprKind::Call(call) => {
                let mut args = Vec::new();
                for arg in &call.args {
                    let value = self
                        .push_typed_exprs_from_expr(arg, exprs, sites, bindings)?
                        .ok_or_else(|| missing_apply_value("tuple argument"))?;
                    args.push(value);
                }
                let callee = self
                    .push_typed_exprs_from_expr(&call.function, exprs, sites, bindings)?
                    .unwrap_or_else(|| self.next_value());
                apply = Some(PendingApply {
                    callee_hint: callee_hint(call.function.as_ref()),
                    apply: self.make_call_apply(call, callee, args)?,
                });
            }
            ExprKind::Pipe(pipe) => {
                let object = self.push_typed_exprs_from_expr(&pipe.expr, exprs, sites, bindings)?;
                match &pipe.target {
                    PipeTarget::Expr(target) => {
                        let object = object.ok_or_else(|| missing_apply_value("pipe object"))?;
                        let callee = self
                            .push_typed_exprs_from_expr(target, exprs, sites, bindings)?
                            .unwrap_or_else(|| self.next_value());
                        apply = Some(PendingApply {
                            callee_hint: callee_hint(target),
                            apply: self.make_pipe_apply(callee, object, target)?,
                        });
                    }
                    PipeTarget::Ident(name) => {
                        if self.checker.checked_function_return_type(name).is_some() {
                            let object =
                                object.ok_or_else(|| missing_apply_value("pipe object"))?;
                            let callee = self.next_value();
                            apply = Some(PendingApply {
                                callee_hint: Some(name.clone()),
                                apply: self.make_top_level_pipe_apply(callee, object, name)?,
                            });
                        }
                    }
                }
            }
            ExprKind::RecordLit(record) => {
                for field in &record.fields {
                    self.push_typed_exprs_from_field_init(field, exprs, sites, bindings)?;
                }
            }
            ExprKind::Clone(clone) => {
                self.push_typed_exprs_from_expr(&clone.base, exprs, sites, bindings)?;
                for field in &clone.updates.fields {
                    self.push_typed_exprs_from_field_init(field, exprs, sites, bindings)?;
                }
            }
            ExprKind::Freeze(inner)
            | ExprKind::Await(inner)
            | ExprKind::Spawn(inner)
            | ExprKind::Some(inner)
            | ExprKind::Ok(inner)
            | ExprKind::Err(inner)
            | ExprKind::FieldAccess(inner, _) => {
                self.push_typed_exprs_from_expr(inner, exprs, sites, bindings)?;
            }
            ExprKind::PrototypeClone(clone) => {
                for field in &clone.updates.fields {
                    self.push_typed_exprs_from_field_init(field, exprs, sites, bindings)?;
                }
            }
            ExprKind::Then(then) => {
                self.push_typed_exprs_from_expr(&then.condition, exprs, sites, bindings)?;
                self.push_typed_exprs_from_block(&then.then_block, exprs, sites, bindings)?;
                for (condition, block) in &then.else_ifs {
                    self.push_typed_exprs_from_expr(condition, exprs, sites, bindings)?;
                    self.push_typed_exprs_from_block(block, exprs, sites, bindings)?;
                }
                if let Some(block) = &then.else_block {
                    self.push_typed_exprs_from_block(block, exprs, sites, bindings)?;
                }
            }
            ExprKind::While(while_expr) => {
                self.push_typed_exprs_from_expr(&while_expr.condition, exprs, sites, bindings)?;
                self.push_typed_exprs_from_block(&while_expr.body, exprs, sites, bindings)?;
            }
            ExprKind::Match(match_expr) => {
                self.push_typed_exprs_from_expr(&match_expr.expr, exprs, sites, bindings)?;
                for arm in &match_expr.arms {
                    self.push_typed_exprs_from_block(&arm.body, exprs, sites, bindings)?;
                }
            }
            ExprKind::Binary(binary) => {
                self.push_typed_exprs_from_expr(&binary.left, exprs, sites, bindings)?;
                self.push_typed_exprs_from_expr(&binary.right, exprs, sites, bindings)?;
            }
            ExprKind::Unary(unary) => {
                self.push_typed_exprs_from_expr(&unary.expr, exprs, sites, bindings)?;
            }
            ExprKind::Cast(cast) => {
                self.push_typed_exprs_from_expr(&cast.expr, exprs, sites, bindings)?;
            }
            ExprKind::With(with) => {
                for binding in &with.bindings {
                    self.push_typed_exprs_from_field_init(binding, exprs, sites, bindings)?;
                }
                self.push_typed_exprs_from_block(&with.body, exprs, sites, bindings)?;
            }
            ExprKind::WithLifetime(with) => {
                self.push_typed_exprs_from_block(&with.body, exprs, sites, bindings)?;
            }
            ExprKind::Block(block) => {
                self.push_typed_exprs_from_block(block, exprs, sites, bindings)?;
            }
            ExprKind::ListLit(items) | ExprKind::ArrayLit(items) => {
                for item in items {
                    self.push_typed_exprs_from_expr(item, exprs, sites, bindings)?;
                }
            }
            ExprKind::RangeLit(range) => {
                self.push_typed_exprs_from_expr(&range.start, exprs, sites, bindings)?;
                self.push_typed_exprs_from_expr(&range.end, exprs, sites, bindings)?;
            }
            ExprKind::Lambda(lambda) => {
                self.push_typed_exprs_from_expr(&lambda.body, exprs, sites, bindings)?;
            }
            ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::BoolLit(_)
            | ExprKind::Unit
            | ExprKind::Ident(_)
            | ExprKind::None => {}
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
        bindings: &mut Vec<CheckedBindingIr>,
    ) -> Result<(), IrBuildError> {
        match field {
            FieldInit::Field { value, .. } | FieldInit::Spread(value) => {
                self.push_typed_exprs_from_expr(value, exprs, sites, bindings)?;
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
        let repr = self.value_repr_for_type(&final_type);
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
                } else if let ExprKind::Ident(name) = &expr.kind {
                    self.lookup_binding(name)
                        .map(TypedExprKind::Binding)
                        .unwrap_or(TypedExprKind::Value(value))
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

    fn make_call_apply(
        &mut self,
        call: &CallExpr,
        callee: ValueId,
        args: Vec<ValueId>,
    ) -> Result<ApplyIr, IrBuildError> {
        let flavor = match &call.function.kind {
            ExprKind::Lambda(_) => ApplyFlavor::ImmediateLambda,
            ExprKind::FieldAccess(_, _) => ApplyFlavor::MethodResolution,
            ExprKind::Ident(_) if call.args.is_empty() => ApplyFlavor::UnitCall,
            ExprKind::Ident(_) => ApplyFlavor::TupleCall,
            _ => ApplyFlavor::FunctionValue,
        };
        let callee_provenance = self.callee_provenance_for_expr(call.function.as_ref())?;
        let result = self.next_value();

        Ok(ApplyIr {
            flavor,
            callee,
            callee_provenance,
            args,
            result,
        })
    }

    fn make_pipe_apply(
        &mut self,
        callee: ValueId,
        object: ValueId,
        target: &Expr,
    ) -> Result<ApplyIr, IrBuildError> {
        let callee_provenance = self.callee_provenance_for_expr(target)?;
        Ok(self.make_pipe_apply_with_provenance(callee, object, callee_provenance))
    }

    fn make_top_level_pipe_apply(
        &mut self,
        callee: ValueId,
        object: ValueId,
        name: &str,
    ) -> Result<ApplyIr, IrBuildError> {
        let callee_provenance = self
            .top_level_callee_provenance(name)?
            .unwrap_or(CalleeProvenance::Value);
        Ok(self.make_pipe_apply_with_provenance(callee, object, callee_provenance))
    }

    fn make_pipe_apply_with_provenance(
        &mut self,
        callee: ValueId,
        object: ValueId,
        callee_provenance: CalleeProvenance,
    ) -> ApplyIr {
        let result = self.next_value();

        ApplyIr {
            flavor: ApplyFlavor::Pipe,
            callee,
            callee_provenance,
            args: vec![object],
            result,
        }
    }

    fn callee_provenance_for_expr(
        &mut self,
        expr: &Expr,
    ) -> Result<CalleeProvenance, IrBuildError> {
        match &expr.kind {
            ExprKind::Ident(name) => Ok(self
                .top_level_callee_provenance(name)?
                .unwrap_or(CalleeProvenance::Value)),
            _ => Ok(CalleeProvenance::Value),
        }
    }

    fn top_level_callee_provenance(
        &mut self,
        name: &str,
    ) -> Result<Option<CalleeProvenance>, IrBuildError> {
        let Some(signature) = self.checker.checked_function_signature(name) else {
            return Ok(None);
        };

        let params = signature
            .params
            .iter()
            .map(|(_, ty)| FinalType::new(ty.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let return_type = FinalType::new(signature.return_type)?;
        let return_repr = self.value_repr_for_type(&return_type);
        let monomorphic =
            return_type.is_monomorphic() && params.iter().all(FinalType::is_monomorphic);
        let declared_type_params = signature
            .type_params
            .iter()
            .map(format_type_param_name)
            .collect::<Vec<_>>();

        Ok(Some(CalleeProvenance::TopLevelFunction(FunctionCalleeIr {
            name: name.to_string(),
            declared_type_params,
            params,
            return_type,
            return_repr,
            monomorphic,
        })))
    }

    fn start_function_binding_scope(&mut self) {
        self.binding_scopes.clear();
        self.push_binding_scope();
    }

    fn finish_function_binding_scope(&mut self) {
        self.binding_scopes.clear();
    }

    fn push_binding_scope(&mut self) {
        self.binding_scopes.push(HashMap::new());
    }

    fn pop_binding_scope(&mut self) {
        self.binding_scopes.pop();
    }

    fn register_binding_name(&mut self, name: &str, binding: BindingId) {
        if let Some(scope) = self.binding_scopes.last_mut() {
            scope.insert(name.to_string(), BindingScopeEntry::Known(binding));
        }
    }

    fn shadow_binding_name(&mut self, name: &str) {
        if let Some(scope) = self.binding_scopes.last_mut() {
            scope.insert(name.to_string(), BindingScopeEntry::ShadowOnly);
        }
    }

    fn lookup_binding(&self, name: &str) -> Option<BindingId> {
        for scope in self.binding_scopes.iter().rev() {
            match scope.get(name) {
                Some(BindingScopeEntry::Known(binding)) => return Some(*binding),
                Some(BindingScopeEntry::ShadowOnly) => return None,
                None => {}
            }
        }
        None
    }

    fn register_local_binding(
        &mut self,
        binding: &BindDecl,
        value: Option<ValueId>,
        exprs: &[TypedExpr],
        bindings: &mut Vec<CheckedBindingIr>,
    ) -> Result<(), IrBuildError> {
        let Pattern::Ident(name) = &binding.pattern else {
            for name in pattern_bound_names(&binding.pattern) {
                self.shadow_binding_name(&name);
            }
            return Ok(());
        };
        let Some(value) = value else {
            return Ok(());
        };
        let Some(producer) = producer_for_value(exprs, value) else {
            return Ok(());
        };

        let id = self.next_binding_id();
        self.register_binding_name(name, id);
        bindings.push(CheckedBindingIr {
            id,
            name: name.clone(),
            mutable: binding.mutable,
            source: CheckedBindingSource::Local,
            value: Some(value),
            final_type: producer.final_type.clone(),
            repr: producer.repr,
        });

        Ok(())
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

    fn next_binding_id(&mut self) -> BindingId {
        let id = BindingId(self.next_binding);
        self.next_binding += 1;
        id
    }

    fn next_apply_index(&mut self) -> usize {
        let index = self.next_apply;
        self.next_apply += 1;
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_shadows_complex_pattern_names_without_binding_provenance() {
        let checker = TypeChecker::new();
        let mut builder = CheckedIrBuilder::new(&checker);
        let mut bindings = Vec::new();
        builder.start_function_binding_scope();
        let outer_alias_binding = builder.next_binding_id();
        builder.register_binding_name("alias", outer_alias_binding);
        builder.push_binding_scope();

        let binding = BindDecl {
            mutable: false,
            pattern: Pattern::Some(Box::new(Pattern::Ident("alias".to_string()))),
            type_annotation: None,
            value: Box::new(Expr::new(ExprKind::None)),
        };
        builder
            .register_local_binding(&binding, None, &[], &mut bindings)
            .expect("complex pattern shadowing should not fail");

        assert!(bindings.is_empty());
        assert_eq!(builder.lookup_binding("alias"), None);
        builder.pop_binding_scope();
        assert_eq!(builder.lookup_binding("alias"), Some(outer_alias_binding));
    }
}
