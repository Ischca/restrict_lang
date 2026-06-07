//! Read-only binding def-use graph derived from Checked IR.
//!
//! This is the first explicit, validated def-use structure for builder-local
//! `BindingId`s. It centralizes, for each binding, its definition site and every
//! read site together with how that read's value is consumed. Later move/copy
//! analyses should consume this graph instead of reconstructing the linking from
//! the flat value list.
//!
//! The graph remains build-local: `BindingId`s are still not stable across
//! builds or a codegen authority. Building this graph does not change generated
//! WAT or authorize any rewrite; it only makes the binding flow explicit.

use std::collections::HashMap;

use super::builder::{CheckedBindingSource, CheckedFunctionIr, CheckedProgramIr};
use super::{BindingId, ExprId, TypedExprKind, UseEvent, ValueId, ValueRepr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingGraph {
    pub functions: Vec<FunctionBindingGraph>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionBindingGraph {
    pub name: String,
    pub bindings: Vec<BindingNode>,
}

impl FunctionBindingGraph {
    pub fn binding(&self, id: BindingId) -> Option<&BindingNode> {
        self.bindings.iter().find(|binding| binding.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingNode {
    pub id: BindingId,
    pub name: String,
    pub mutable: bool,
    pub repr: ValueRepr,
    pub def: BindingDef,
    pub reads: Vec<BindingRead>,
}

/// Where a binding's value originates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingDef {
    /// A function parameter at the given source position.
    Parameter { index: usize },
    /// A simple local `val`/`mut val` binding, carrying the value it was bound to
    /// and the expression that produced that value.
    Local { value: ValueId, producer: ExprId },
}

/// One read of a binding: the `Binding` expression that resolves to it, the value
/// that read produces, and how that produced value is subsequently consumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingRead {
    pub at: ExprId,
    pub value: ValueId,
    pub uses: Vec<UseEvent>,
}

/// A binding-flow inconsistency. These mirror facts the shadow invariant
/// validator already guarantees for a built `CheckedProgramIr`; surfacing them
/// loudly keeps the graph honest if it is ever fed an unvalidated function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingGraphError {
    LocalBindingMissingValue {
        function: String,
        binding: BindingId,
    },
    LocalBindingMissingProducer {
        function: String,
        binding: BindingId,
        value: ValueId,
    },
    BindingReadMissingValue {
        function: String,
        binding: BindingId,
        at: ExprId,
    },
    DanglingBindingRead {
        function: String,
        binding: BindingId,
        at: ExprId,
    },
}

pub fn build_binding_graph(program: &CheckedProgramIr) -> Result<BindingGraph, BindingGraphError> {
    let functions = program
        .functions
        .iter()
        .map(build_function_binding_graph)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BindingGraph { functions })
}

pub fn build_function_binding_graph(
    function: &CheckedFunctionIr,
) -> Result<FunctionBindingGraph, BindingGraphError> {
    let mut uses_by_value: HashMap<ValueId, Vec<UseEvent>> = HashMap::new();
    for expr in &function.typed_exprs {
        for event in expr.flow.uses() {
            uses_by_value.entry(event.value).or_default().push(*event);
        }
    }

    let mut producer_by_value: HashMap<ValueId, ExprId> = HashMap::new();
    for expr in &function.typed_exprs {
        for value in expr.flow.produced() {
            producer_by_value.entry(*value).or_insert(expr.id);
        }
    }

    let mut reads_by_binding: HashMap<BindingId, Vec<BindingRead>> = HashMap::new();
    for expr in &function.typed_exprs {
        if let TypedExprKind::Binding(binding) = &expr.kind {
            let value = expr
                .value
                .ok_or_else(|| BindingGraphError::BindingReadMissingValue {
                    function: function.name.clone(),
                    binding: *binding,
                    at: expr.id,
                })?;
            let uses = uses_by_value.get(&value).cloned().unwrap_or_default();
            reads_by_binding
                .entry(*binding)
                .or_default()
                .push(BindingRead {
                    at: expr.id,
                    value,
                    uses,
                });
        }
    }

    let mut bindings = Vec::new();
    for binding in &function.bindings {
        let def = match binding.source {
            CheckedBindingSource::Param { index } => BindingDef::Parameter { index },
            CheckedBindingSource::Local => {
                let value =
                    binding
                        .value
                        .ok_or_else(|| BindingGraphError::LocalBindingMissingValue {
                            function: function.name.clone(),
                            binding: binding.id,
                        })?;
                let producer = *producer_by_value.get(&value).ok_or_else(|| {
                    BindingGraphError::LocalBindingMissingProducer {
                        function: function.name.clone(),
                        binding: binding.id,
                        value,
                    }
                })?;
                BindingDef::Local { value, producer }
            }
        };
        let reads = reads_by_binding.remove(&binding.id).unwrap_or_default();
        bindings.push(BindingNode {
            id: binding.id,
            name: binding.name.clone(),
            mutable: binding.mutable,
            repr: binding.repr,
            def,
            reads,
        });
    }

    // A read that survives here resolves to a binding this function never
    // declared. The shadow invariant validator rejects that at build time;
    // report it deterministically rather than dropping it silently.
    if let Some(binding) = reads_by_binding.keys().min_by_key(|id| id.0).copied() {
        let at = reads_by_binding[&binding][0].at;
        return Err(BindingGraphError::DanglingBindingRead {
            function: function.name.clone(),
            binding,
            at,
        });
    }

    Ok(FunctionBindingGraph {
        name: function.name.clone(),
        bindings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::builder::{
        build_checked_ir, CheckedBindingIr, CheckedFunctionIr, CheckedFunctionLoweringSummary,
        LoweringReadiness,
    };
    use crate::ir::{FinalType, HostAbi, ScalarRepr, UseKind};
    use crate::parser::parse_program;
    use crate::type_checker::{TypeChecker, TypedType};

    fn function_graph(source: &str, name: &str) -> FunctionBindingGraph {
        let (remaining, program) = parse_program(source).expect("source should parse");
        assert!(remaining.trim().is_empty(), "unparsed input: {remaining:?}");
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("source should type-check");
        let ir = build_checked_ir(&program, &checker).expect("checked IR should build");
        let graph = build_binding_graph(&ir).expect("binding graph should build");
        graph
            .functions
            .into_iter()
            .find(|function| function.name == name)
            .expect("function graph should be present")
    }

    #[test]
    fn binding_graph_links_param_local_def_and_reads() {
        let graph = function_graph(
            r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: (items: List<Int32>) -> List<Int32> = {
    val alias = items
    alias |> keep
}
"#,
            "main",
        );

        let items = graph
            .bindings
            .iter()
            .find(|binding| binding.name == "items")
            .expect("param binding should be present");
        assert_eq!(items.def, BindingDef::Parameter { index: 0 });
        assert!(!items.mutable);
        // `items` is read once - where `alias` is bound to it.
        assert_eq!(items.reads.len(), 1);
        let items_read = &items.reads[0];

        let alias = graph
            .bindings
            .iter()
            .find(|binding| binding.name == "alias")
            .expect("local binding should be present");
        // The local's bound value is exactly the value the `items` read produced.
        match alias.def {
            BindingDef::Local { value, producer } => {
                assert_eq!(value, items_read.value);
                assert_eq!(producer, items_read.at);
            }
            BindingDef::Parameter { .. } => panic!("alias should be a local binding"),
        }

        // `alias` is read once and that read's value is moved into the pipe apply.
        assert_eq!(alias.reads.len(), 1);
        let alias_read = &alias.reads[0];
        assert_eq!(alias_read.uses.len(), 1);
        assert_eq!(alias_read.uses[0].kind, UseKind::Move);
        assert_eq!(alias_read.uses[0].value, alias_read.value);
    }

    #[test]
    fn binding_graph_is_address_independent() {
        let graph_a = function_graph(
            r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: (items: List<Int32>) -> List<Int32> = {
    items |> keep
}
"#,
            "main",
        );
        let graph_b = function_graph(
            r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: (items: List<Int32>) -> List<Int32> = {
    items |> keep
}
"#,
            "main",
        );
        assert_eq!(graph_a, graph_b);
    }

    #[test]
    fn binding_graph_rejects_local_binding_without_value() {
        // A local binding must carry the value it was bound to; a missing value
        // is an invariant violation, not a silently empty definition.
        let function = CheckedFunctionIr {
            name: "main".to_string(),
            params: Vec::new(),
            return_type: FinalType::new(TypedType::Int32).unwrap(),
            return_repr: ValueRepr::Scalar(ScalarRepr::I32),
            bindings: vec![CheckedBindingIr {
                id: BindingId(0),
                name: "alias".to_string(),
                mutable: false,
                source: CheckedBindingSource::Local,
                value: None,
                final_type: FinalType::new(TypedType::Int32).unwrap(),
                repr: ValueRepr::Scalar(ScalarRepr::I32),
            }],
            apply_sites: Vec::new(),
            typed_exprs: Vec::new(),
            monomorphic: true,
            lowering: CheckedFunctionLoweringSummary {
                source_exported: false,
                declared_type_params: Vec::new(),
                temporal_constraints: Vec::new(),
                param_host_abis: Vec::new(),
                return_host_abi: HostAbi::Scalar(ScalarRepr::I32),
                body_result: None,
                required_layouts: Vec::new(),
                readiness: LoweringReadiness {
                    v001_host_abi_eligible: true,
                    internal_layout_ready: true,
                    host_abi_blockers: Vec::new(),
                    internal_lowering_blockers: Vec::new(),
                },
            },
        };

        assert_eq!(
            build_function_binding_graph(&function),
            Err(BindingGraphError::LocalBindingMissingValue {
                function: "main".to_string(),
                binding: BindingId(0),
            })
        );
    }
}
