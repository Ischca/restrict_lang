//! Read-only Checked IR builder.
//!
//! This builder intentionally shadows the existing AST-driven codegen pipeline.
//! It does not re-run inference, does not inspect or mutate affine checker
//! state, and is not yet the codegen source of truth.

use crate::ast::*;
use crate::type_checker::{CheckedFunctionSignature, TypeChecker};

use super::layout::LayoutTable;
use super::{ApplyFlavor, ApplyIr, FinalType, ValueId, ValueRepr};

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
    pub callee_hint: Option<String>,
    pub apply: ApplyIr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrBuildError {
    MissingCheckedReturn(String),
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
    next_value: u32,
    next_apply: usize,
}

impl<'a> CheckedIrBuilder<'a> {
    fn new(checker: &'a TypeChecker) -> Self {
        Self {
            checker,
            layout_table: LayoutTable::new(),
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
        self.collect_applies_from_block(&func.body, &mut apply_sites);

        let monomorphic = return_type.is_monomorphic()
            && params.iter().all(|param| param.final_type.is_monomorphic());

        Ok(CheckedFunctionIr {
            name: func.name.clone(),
            params,
            return_type,
            return_repr,
            apply_sites,
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

    fn collect_applies_from_block(
        &mut self,
        block: &BlockExpr,
        sites: &mut Vec<NormalizedApplySite>,
    ) {
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(binding) => self.collect_applies_from_expr(&binding.value, sites),
                Stmt::Assignment(assign) => self.collect_applies_from_expr(&assign.value, sites),
                Stmt::Expr(expr) => self.collect_applies_from_expr(expr, sites),
            }
        }
        if let Some(expr) = &block.expr {
            self.collect_applies_from_expr(expr, sites);
        }
    }

    fn collect_applies_from_expr(&mut self, expr: &Expr, sites: &mut Vec<NormalizedApplySite>) {
        match expr {
            Expr::Call(call) => {
                for arg in &call.args {
                    self.collect_applies_from_expr(arg, sites);
                }
                self.collect_applies_from_expr(&call.function, sites);
                self.push_call_apply(call, sites);
            }
            Expr::Pipe(pipe) => {
                self.collect_applies_from_expr(&pipe.expr, sites);
                if let PipeTarget::Expr(target) = &pipe.target {
                    self.collect_applies_from_expr(target, sites);
                    self.push_pipe_apply(pipe, sites);
                } else if let PipeTarget::Ident(name) = &pipe.target {
                    if self.checker.checked_function_return_type(name).is_some() {
                        self.push_pipe_apply(pipe, sites);
                    }
                }
            }
            Expr::RecordLit(record) => {
                for field in &record.fields {
                    self.collect_applies_from_field_init(field, sites);
                }
            }
            Expr::Clone(clone) => {
                self.collect_applies_from_expr(&clone.base, sites);
                for field in &clone.updates.fields {
                    self.collect_applies_from_field_init(field, sites);
                }
            }
            Expr::Freeze(inner)
            | Expr::Await(inner)
            | Expr::Spawn(inner)
            | Expr::Some(inner)
            | Expr::Ok(inner)
            | Expr::Err(inner)
            | Expr::FieldAccess(inner, _) => self.collect_applies_from_expr(inner, sites),
            Expr::PrototypeClone(clone) => {
                for field in &clone.updates.fields {
                    self.collect_applies_from_field_init(field, sites);
                }
            }
            Expr::Then(then) => {
                self.collect_applies_from_expr(&then.condition, sites);
                self.collect_applies_from_block(&then.then_block, sites);
                for (condition, block) in &then.else_ifs {
                    self.collect_applies_from_expr(condition, sites);
                    self.collect_applies_from_block(block, sites);
                }
                if let Some(block) = &then.else_block {
                    self.collect_applies_from_block(block, sites);
                }
            }
            Expr::While(while_expr) => {
                self.collect_applies_from_expr(&while_expr.condition, sites);
                self.collect_applies_from_block(&while_expr.body, sites);
            }
            Expr::Match(match_expr) => {
                self.collect_applies_from_expr(&match_expr.expr, sites);
                for arm in &match_expr.arms {
                    self.collect_applies_from_block(&arm.body, sites);
                }
            }
            Expr::Binary(binary) => {
                self.collect_applies_from_expr(&binary.left, sites);
                self.collect_applies_from_expr(&binary.right, sites);
            }
            Expr::Unary(unary) => self.collect_applies_from_expr(&unary.expr, sites),
            Expr::Cast(cast) => self.collect_applies_from_expr(&cast.expr, sites),
            Expr::With(with) => {
                for binding in &with.bindings {
                    self.collect_applies_from_field_init(binding, sites);
                }
                self.collect_applies_from_block(&with.body, sites);
            }
            Expr::WithLifetime(with) => self.collect_applies_from_block(&with.body, sites),
            Expr::Block(block) => self.collect_applies_from_block(block, sites),
            Expr::ListLit(items) | Expr::ArrayLit(items) => {
                for item in items {
                    self.collect_applies_from_expr(item, sites);
                }
            }
            Expr::RangeLit(range) => {
                self.collect_applies_from_expr(&range.start, sites);
                self.collect_applies_from_expr(&range.end, sites);
            }
            Expr::Lambda(lambda) => self.collect_applies_from_expr(&lambda.body, sites),
            Expr::IntLit(_)
            | Expr::FloatLit(_)
            | Expr::StringLit(_)
            | Expr::CharLit(_)
            | Expr::BoolLit(_)
            | Expr::Unit
            | Expr::Ident(_)
            | Expr::None => {}
        }
    }

    fn collect_applies_from_field_init(
        &mut self,
        field: &FieldInit,
        sites: &mut Vec<NormalizedApplySite>,
    ) {
        match field {
            FieldInit::Field { value, .. } | FieldInit::Spread(value) => {
                self.collect_applies_from_expr(value, sites)
            }
        }
    }

    fn push_call_apply(&mut self, call: &CallExpr, sites: &mut Vec<NormalizedApplySite>) {
        let flavor = match call.function.as_ref() {
            Expr::Lambda(_) => ApplyFlavor::ImmediateLambda,
            Expr::FieldAccess(_, _) => ApplyFlavor::MethodResolution,
            Expr::Ident(_) if call.args.is_empty() => ApplyFlavor::UnitCall,
            Expr::Ident(_) => ApplyFlavor::TupleCall,
            _ => ApplyFlavor::FunctionValue,
        };
        let callee_hint = callee_hint(call.function.as_ref());
        let callee = self.next_value();
        let args = (0..call.args.len())
            .map(|_| self.next_value())
            .collect::<Vec<_>>();
        let result = self.next_value();
        let source_index = self.next_apply_index();

        sites.push(NormalizedApplySite {
            source_index,
            callee_hint,
            apply: ApplyIr {
                flavor,
                callee,
                args,
                result,
            },
        });
    }

    fn push_pipe_apply(&mut self, pipe: &PipeExpr, sites: &mut Vec<NormalizedApplySite>) {
        let callee_hint = match &pipe.target {
            PipeTarget::Ident(name) => Some(name.clone()),
            PipeTarget::Expr(expr) => callee_hint(expr),
        };
        let callee = self.next_value();
        let arg = self.next_value();
        let result = self.next_value();
        let source_index = self.next_apply_index();

        sites.push(NormalizedApplySite {
            source_index,
            callee_hint,
            apply: ApplyIr {
                flavor: ApplyFlavor::Pipe,
                callee,
                args: vec![arg],
                result,
            },
        });
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
}
