//! Typed IR foundation for Restrict.
//!
//! This module is intentionally not wired into code generation yet. It defines
//! the boundary we want between finalized type inference, affine flow checking,
//! layout selection, and later WebAssembly lowering.

pub mod builder;
pub mod layout;
pub mod optimize;

use crate::type_checker::{format_typed_type, TypedType};
use layout::LayoutId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbiId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct FinalType {
    ty: TypedType,
}

impl FinalType {
    pub fn new(ty: TypedType) -> Result<Self, IrError> {
        if contains_inference_type(&ty) {
            return Err(IrError::UnfinalizedType(format_typed_type(&ty)));
        }
        Ok(Self { ty })
    }

    pub fn as_typed_type(&self) -> &TypedType {
        &self.ty
    }

    pub fn into_typed_type(self) -> TypedType {
        self.ty
    }

    pub fn is_monomorphic(&self) -> bool {
        !contains_type_param(&self.ty)
    }

    pub fn host_abi(&self) -> HostAbi {
        HostAbi::for_type(&self.ty)
    }
}

fn contains_inference_type(ty: &TypedType) -> bool {
    match ty {
        TypedType::InferVar(_) | TypedType::Projection { .. } => true,
        TypedType::Record { type_args, .. } => type_args.iter().any(contains_inference_type),
        TypedType::Function {
            params,
            return_type,
        } => params.iter().any(contains_inference_type) || contains_inference_type(return_type),
        TypedType::Option(inner)
        | TypedType::List(inner)
        | TypedType::Array(inner, _)
        | TypedType::Temporal {
            base_type: inner, ..
        } => contains_inference_type(inner),
        TypedType::Result(ok, err) => contains_inference_type(ok) || contains_inference_type(err),
        _ => false,
    }
}

fn contains_type_param(ty: &TypedType) -> bool {
    match ty {
        TypedType::TypeParam(_) => true,
        TypedType::Record { type_args, .. } => type_args.iter().any(contains_type_param),
        TypedType::Function {
            params,
            return_type,
        } => params.iter().any(contains_type_param) || contains_type_param(return_type),
        TypedType::Option(inner)
        | TypedType::List(inner)
        | TypedType::Array(inner, _)
        | TypedType::Temporal {
            base_type: inner, ..
        } => contains_type_param(inner),
        TypedType::Result(ok, err) => contains_type_param(ok) || contains_type_param(err),
        TypedType::InferVar(_) | TypedType::Projection { .. } => true,
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarRepr {
    I32,
    I64,
    F64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueRepr {
    Unit,
    Scalar(ScalarRepr),
    Ref(LayoutId),
    Closure { layout: LayoutId, abi: AbiId },
}

impl ValueRepr {
    pub fn is_runtime_reference(self) -> bool {
        matches!(self, ValueRepr::Ref(_) | ValueRepr::Closure { .. })
    }

    pub fn is_copy_scalar(self) -> bool {
        matches!(self, ValueRepr::Scalar(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAbi {
    Unit,
    Scalar(ScalarRepr),
    InternalOnly(InternalOnlyReason),
}

impl HostAbi {
    pub fn for_type(ty: &TypedType) -> Self {
        match ty {
            TypedType::Unit => HostAbi::Unit,
            TypedType::Int32 | TypedType::Boolean | TypedType::Char => {
                HostAbi::Scalar(ScalarRepr::I32)
            }
            TypedType::Int64 => HostAbi::Scalar(ScalarRepr::I64),
            TypedType::Float64 => HostAbi::Scalar(ScalarRepr::F64),
            TypedType::String
            | TypedType::Record { .. }
            | TypedType::Function { .. }
            | TypedType::Option(_)
            | TypedType::Result(_, _)
            | TypedType::List(_)
            | TypedType::Array(_, _)
            | TypedType::Temporal { .. } => {
                HostAbi::InternalOnly(InternalOnlyReason::CompositeHostAbiUnstable)
            }
            TypedType::TypeParam(_) => HostAbi::InternalOnly(InternalOnlyReason::GenericType),
            TypedType::InferVar(_) | TypedType::Projection { .. } => {
                HostAbi::InternalOnly(InternalOnlyReason::UnfinalizedType)
            }
        }
    }

    pub fn is_v001_exportable(&self) -> bool {
        matches!(self, HostAbi::Unit | HostAbi::Scalar(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternalOnlyReason {
    CompositeHostAbiUnstable,
    GenericType,
    UnfinalizedType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionKind {
    DefaultArena,
    ArenaScope,
    HostBoundary,
    TemporalScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub id: RegionId,
    pub kind: RegionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseKind {
    ReadCopy,
    Move,
    BorrowShared,
    BorrowMut,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseEvent {
    pub value: ValueId,
    pub kind: UseKind,
    pub at: ExprId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowSummary {
    uses: Vec<UseEvent>,
    produced: Vec<ValueId>,
}

impl FlowSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_use(&mut self, event: UseEvent) {
        self.uses.push(event);
    }

    pub fn record_produced(&mut self, value: ValueId) {
        self.produced.push(value);
    }

    pub fn uses(&self) -> &[UseEvent] {
        &self.uses
    }

    pub fn produced(&self) -> &[ValueId] {
        &self.produced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyFlavor {
    Pipe,
    TupleCall,
    UnitCall,
    FunctionValue,
    ImmediateLambda,
    MethodResolution,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CalleeProvenance {
    TopLevelFunction(FunctionCalleeIr),
    Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCalleeIr {
    pub name: String,
    pub declared_type_params: Vec<String>,
    pub params: Vec<FinalType>,
    pub return_type: FinalType,
    pub return_repr: ValueRepr,
    pub monomorphic: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyIr {
    pub flavor: ApplyFlavor,
    pub callee: ValueId,
    pub callee_provenance: CalleeProvenance,
    pub args: Vec<ValueId>,
    pub result: ValueId,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypedExprKind {
    Literal,
    Value(ValueId),
    Binding(BindingId),
    Apply(ApplyIr),
    Block(Vec<TypedExpr>),
    Branch {
        condition: ValueId,
        then_expr: Box<TypedExpr>,
        else_expr: Box<TypedExpr>,
    },
    Match {
        scrutinee: ValueId,
        arms: Vec<TypedExpr>,
    },
    Region {
        region: Region,
        body: Box<TypedExpr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedExpr {
    pub id: ExprId,
    pub value: Option<ValueId>,
    pub final_type: FinalType,
    pub repr: ValueRepr,
    pub flow: FlowSummary,
    pub kind: TypedExprKind,
}

impl TypedExpr {
    pub fn validate_for_codegen(&self) -> Result<(), IrError> {
        if self.final_type.host_abi() == HostAbi::InternalOnly(InternalOnlyReason::UnfinalizedType)
        {
            return Err(IrError::UnfinalizedType(format_typed_type(
                self.final_type.as_typed_type(),
            )));
        }

        validate_flow(&self.flow)?;

        match &self.kind {
            TypedExprKind::Block(exprs) => {
                for expr in exprs {
                    expr.validate_for_codegen()?;
                }
            }
            TypedExprKind::Branch {
                then_expr,
                else_expr,
                ..
            } => {
                then_expr.validate_for_codegen()?;
                else_expr.validate_for_codegen()?;
            }
            TypedExprKind::Match { arms, .. } => {
                for arm in arms {
                    arm.validate_for_codegen()?;
                }
            }
            TypedExprKind::Region { body, .. } => body.validate_for_codegen()?,
            _ => {}
        }

        Ok(())
    }
}

fn validate_flow(flow: &FlowSummary) -> Result<(), IrError> {
    let mut moved = Vec::new();
    for event in flow.uses() {
        match event.kind {
            UseKind::Move | UseKind::Drop => {
                if moved.contains(&event.value) {
                    return Err(IrError::AffineDoubleUse(event.value));
                }
                moved.push(event.value);
            }
            UseKind::ReadCopy | UseKind::BorrowShared | UseKind::BorrowMut => {}
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrError {
    UnfinalizedType(String),
    AffineDoubleUse(ValueId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_constraints::TypeVarId;

    #[test]
    fn final_type_rejects_inference_variables() {
        let err = FinalType::new(TypedType::InferVar(TypeVarId(7))).unwrap_err();
        assert_eq!(err, IrError::UnfinalizedType("?7".to_string()));
    }

    #[test]
    fn final_type_rejects_nested_projection() {
        let err = FinalType::new(TypedType::Option(Box::new(TypedType::Projection {
            base: Box::new(TypedType::List(Box::new(TypedType::Int32))),
            form_name: "Container".to_string(),
            assoc_name: "Mapped".to_string(),
            args: vec![TypedType::String],
        })))
        .unwrap_err();

        assert!(matches!(err, IrError::UnfinalizedType(_)));
    }

    #[test]
    fn host_abi_keeps_v001_scalar_boundary() {
        let int = FinalType::new(TypedType::Int32).unwrap();
        assert_eq!(int.host_abi(), HostAbi::Scalar(ScalarRepr::I32));
        assert!(int.host_abi().is_v001_exportable());

        let list = FinalType::new(TypedType::List(Box::new(TypedType::Int32))).unwrap();
        assert_eq!(
            list.host_abi(),
            HostAbi::InternalOnly(InternalOnlyReason::CompositeHostAbiUnstable)
        );
        assert!(!list.host_abi().is_v001_exportable());
    }

    #[test]
    fn host_abi_v001_exportable_scalar_matrix() {
        let exportable = [
            (TypedType::Unit, HostAbi::Unit),
            (TypedType::Int32, HostAbi::Scalar(ScalarRepr::I32)),
            (TypedType::Int64, HostAbi::Scalar(ScalarRepr::I64)),
            (TypedType::Float64, HostAbi::Scalar(ScalarRepr::F64)),
            (TypedType::Boolean, HostAbi::Scalar(ScalarRepr::I32)),
            (TypedType::Char, HostAbi::Scalar(ScalarRepr::I32)),
        ];

        for (ty, abi) in exportable {
            let final_type = FinalType::new(ty).unwrap();
            assert_eq!(final_type.host_abi(), abi);
            assert!(final_type.host_abi().is_v001_exportable());
        }
    }

    #[test]
    fn host_abi_internal_only_matrix() {
        let composite = InternalOnlyReason::CompositeHostAbiUnstable;
        let internal_only = [
            (TypedType::String, composite.clone()),
            (
                TypedType::Record {
                    name: "ReleaseSlice".to_string(),
                    type_args: Vec::new(),
                    frozen: false,
                    hash: None,
                    parent_hash: None,
                },
                composite.clone(),
            ),
            (
                TypedType::Function {
                    params: vec![TypedType::Int32],
                    return_type: Box::new(TypedType::Int32),
                },
                composite.clone(),
            ),
            (
                TypedType::Option(Box::new(TypedType::Int32)),
                composite.clone(),
            ),
            (
                TypedType::Result(Box::new(TypedType::Int32), Box::new(TypedType::Int32)),
                composite.clone(),
            ),
            (
                TypedType::List(Box::new(TypedType::Int32)),
                composite.clone(),
            ),
            (
                TypedType::Array(
                    Box::new(TypedType::Int32),
                    crate::type_checker::ArrayLength::Known(2),
                ),
                composite.clone(),
            ),
            (
                TypedType::Temporal {
                    base_type: Box::new(TypedType::Int32),
                    temporals: vec!["t".to_string()],
                },
                composite,
            ),
            (
                TypedType::TypeParam("T".to_string()),
                InternalOnlyReason::GenericType,
            ),
        ];

        for (ty, reason) in internal_only {
            let final_type = FinalType::new(ty).unwrap();
            assert_eq!(final_type.host_abi(), HostAbi::InternalOnly(reason));
            assert!(!final_type.host_abi().is_v001_exportable());
        }
    }

    #[test]
    fn final_type_distinguishes_generic_from_monomorphic() {
        let generic = FinalType::new(TypedType::TypeParam("T".to_string())).unwrap();
        assert!(!generic.is_monomorphic());

        let concrete = FinalType::new(TypedType::List(Box::new(TypedType::Int32))).unwrap();
        assert!(concrete.is_monomorphic());
    }

    #[test]
    fn flow_summary_rejects_double_move() {
        let mut flow = FlowSummary::new();
        flow.record_use(UseEvent {
            value: ValueId(1),
            kind: UseKind::Move,
            at: ExprId(10),
        });
        flow.record_use(UseEvent {
            value: ValueId(1),
            kind: UseKind::Move,
            at: ExprId(11),
        });

        assert_eq!(
            validate_flow(&flow),
            Err(IrError::AffineDoubleUse(ValueId(1)))
        );
    }

    #[test]
    fn flow_summary_allows_repeated_copy_reads() {
        let mut flow = FlowSummary::new();
        flow.record_use(UseEvent {
            value: ValueId(1),
            kind: UseKind::ReadCopy,
            at: ExprId(10),
        });
        flow.record_use(UseEvent {
            value: ValueId(1),
            kind: UseKind::ReadCopy,
            at: ExprId(11),
        });

        assert_eq!(validate_flow(&flow), Ok(()));
    }

    #[test]
    fn flow_summary_rejects_move_then_drop() {
        let mut flow = FlowSummary::new();
        flow.record_use(UseEvent {
            value: ValueId(1),
            kind: UseKind::Move,
            at: ExprId(10),
        });
        flow.record_use(UseEvent {
            value: ValueId(1),
            kind: UseKind::Drop,
            at: ExprId(11),
        });

        assert_eq!(
            validate_flow(&flow),
            Err(IrError::AffineDoubleUse(ValueId(1)))
        );
    }

    #[test]
    fn validate_for_codegen_recurses_into_nested_exprs() {
        let final_type = FinalType::new(TypedType::Unit).unwrap();
        let mut bad_flow = FlowSummary::new();
        bad_flow.record_use(UseEvent {
            value: ValueId(7),
            kind: UseKind::Move,
            at: ExprId(2),
        });
        bad_flow.record_use(UseEvent {
            value: ValueId(7),
            kind: UseKind::Move,
            at: ExprId(3),
        });

        let child = TypedExpr {
            id: ExprId(2),
            value: None,
            final_type: final_type.clone(),
            repr: ValueRepr::Unit,
            flow: bad_flow,
            kind: TypedExprKind::Literal,
        };
        let parent = TypedExpr {
            id: ExprId(1),
            value: None,
            final_type,
            repr: ValueRepr::Unit,
            flow: FlowSummary::new(),
            kind: TypedExprKind::Block(vec![child]),
        };

        assert_eq!(
            parent.validate_for_codegen(),
            Err(IrError::AffineDoubleUse(ValueId(7)))
        );
    }
}
