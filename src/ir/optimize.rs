//! Low-level optimization stage for the future Wasm MIR pipeline.

use std::collections::HashMap;

use super::builder::{CheckedBindingIr, CheckedFunctionIr, CheckedProgramIr};
use super::layout::{LayoutId, LayoutKind, LayoutTable, SumOptimizationCandidate, SumStrategy};
use super::{ApplyFlavor, BindingId, ExprId, TypedExprKind, UseEvent, UseKind, ValueId, ValueRepr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramValueUseSummary {
    pub functions: Vec<FunctionValueUseSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionValueUseSummary {
    pub name: String,
    pub values: Vec<ValueUseSummary>,
    pub findings: Vec<ValueUseFinding>,
    pub forwarding_candidates: Vec<AffineForwardingCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueUseSummary {
    pub value: ValueId,
    pub producer: ExprId,
    pub producer_kind: ValueProducerKind,
    pub binding: Option<BindingId>,
    pub repr: ValueRepr,
    pub is_body_result: bool,
    pub uses: Vec<UseEvent>,
    pub classification: ValueUseClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueProducerKind {
    Literal,
    PlainValue,
    Binding,
    Apply,
    Block,
    Branch,
    Match,
    Region,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueUseClassification {
    BodyResult,
    CopyOnly {
        reads: usize,
    },
    SingleMove,
    AffineConsumed {
        copy_reads: usize,
        moves: usize,
        borrows: usize,
        drops: usize,
    },
    UnusedPureValue,
    NotRewritableApply {
        reason: ApplyRewriteBlocker,
    },
    UnusedValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyRewriteBlocker {
    EffectUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueUseFinding {
    CopyOnly {
        value: ValueId,
        reads: usize,
    },
    SingleMove {
        value: ValueId,
        repr: ValueRepr,
    },
    UnusedPureValue {
        value: ValueId,
        producer: ExprId,
    },
    NotRewritableApply {
        value: ValueId,
        producer: ExprId,
        reason: ApplyRewriteBlocker,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffineForwardingCandidate {
    pub value: ValueId,
    pub binding: BindingId,
    pub binding_name: String,
    pub producer: ExprId,
    pub apply_expr: ExprId,
    pub apply_flavor: ApplyFlavor,
    pub arg_index: usize,
    pub repr: ValueRepr,
    pub rewrite_blocker: ForwardingRewriteBlocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardingRewriteBlocker {
    StableBindingGraphRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramLayoutOptimizationSummary {
    pub functions: Vec<FunctionLayoutOptimizationSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLayoutOptimizationSummary {
    pub name: String,
    pub sum_candidates: Vec<SumLayoutOptimizationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SumLayoutOptimizationCandidate {
    pub layout: LayoutId,
    pub strategy: SumStrategy,
    pub candidates: Vec<SumOptimizationCandidate>,
    pub rewrite_blocker: LayoutOptimizationRewriteBlocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutOptimizationRewriteBlocker {
    AdvisoryOnly,
}

pub fn summarize_checked_program_value_uses(program: &CheckedProgramIr) -> ProgramValueUseSummary {
    ProgramValueUseSummary {
        functions: program
            .functions
            .iter()
            .map(summarize_checked_function_value_uses)
            .collect(),
    }
}

pub fn summarize_checked_program_layout_optimizations(
    program: &CheckedProgramIr,
) -> ProgramLayoutOptimizationSummary {
    ProgramLayoutOptimizationSummary {
        functions: program
            .functions
            .iter()
            .map(|function| {
                summarize_checked_function_layout_optimizations(function, &program.layout_table)
            })
            .collect(),
    }
}

pub fn summarize_checked_function_layout_optimizations(
    function: &CheckedFunctionIr,
    layout_table: &LayoutTable,
) -> FunctionLayoutOptimizationSummary {
    let mut sum_candidates = Vec::new();

    for layout in &function.lowering.required_layouts {
        let Some(descriptor) = layout_table.get(*layout) else {
            continue;
        };
        let LayoutKind::Sum(sum) = &descriptor.kind else {
            continue;
        };
        if sum.optimization_candidates.is_empty() {
            continue;
        }
        sum_candidates.push(SumLayoutOptimizationCandidate {
            layout: *layout,
            strategy: sum.strategy.clone(),
            candidates: sum.optimization_candidates.clone(),
            rewrite_blocker: LayoutOptimizationRewriteBlocker::AdvisoryOnly,
        });
    }

    FunctionLayoutOptimizationSummary {
        name: function.name.clone(),
        sum_candidates,
    }
}

pub fn summarize_checked_function_value_uses(
    function: &CheckedFunctionIr,
) -> FunctionValueUseSummary {
    let mut values = Vec::new();
    let mut value_indexes = HashMap::new();
    let body_result = function.lowering.body_result;
    let mut applies_by_expr = HashMap::new();
    let bindings_by_id = function
        .bindings
        .iter()
        .map(|binding| (binding.id, binding))
        .collect::<HashMap<_, _>>();

    for expr in &function.typed_exprs {
        if let TypedExprKind::Apply(apply) = &expr.kind {
            applies_by_expr.insert(expr.id, apply);
        }
        for value in expr.flow.produced() {
            if value_indexes.contains_key(value) {
                continue;
            }
            value_indexes.insert(*value, values.len());
            values.push(ValueUseSummary {
                value: *value,
                producer: expr.id,
                producer_kind: producer_kind(&expr.kind),
                binding: binding_for_kind(&expr.kind),
                repr: expr.repr,
                is_body_result: Some(*value) == body_result,
                uses: Vec::new(),
                classification: ValueUseClassification::UnusedValue,
            });
        }
    }

    for expr in &function.typed_exprs {
        for event in expr.flow.uses() {
            if let Some(index) = value_indexes.get(&event.value) {
                values[*index].uses.push(*event);
            }
        }
    }

    let mut findings = Vec::new();
    for value in &mut values {
        value.classification = classify_value_use(value);
        if let Some(finding) = finding_for_value(value) {
            findings.push(finding);
        }
    }
    let forwarding_candidates =
        forwarding_candidates_for_values(&values, &applies_by_expr, &bindings_by_id);

    FunctionValueUseSummary {
        name: function.name.clone(),
        values,
        findings,
        forwarding_candidates,
    }
}

fn producer_kind(kind: &TypedExprKind) -> ValueProducerKind {
    match kind {
        TypedExprKind::Literal => ValueProducerKind::Literal,
        TypedExprKind::Value(_) => ValueProducerKind::PlainValue,
        TypedExprKind::Binding(_) => ValueProducerKind::Binding,
        TypedExprKind::Apply(_) => ValueProducerKind::Apply,
        TypedExprKind::Block(_) => ValueProducerKind::Block,
        TypedExprKind::Branch { .. } => ValueProducerKind::Branch,
        TypedExprKind::Match { .. } => ValueProducerKind::Match,
        TypedExprKind::Region { .. } => ValueProducerKind::Region,
    }
}

fn binding_for_kind(kind: &TypedExprKind) -> Option<BindingId> {
    match kind {
        TypedExprKind::Binding(binding) => Some(*binding),
        _ => None,
    }
}

fn classify_value_use(value: &ValueUseSummary) -> ValueUseClassification {
    if value.is_body_result {
        return ValueUseClassification::BodyResult;
    }

    if value.uses.is_empty() {
        return match value.producer_kind {
            ValueProducerKind::Literal | ValueProducerKind::PlainValue => {
                ValueUseClassification::UnusedPureValue
            }
            ValueProducerKind::Apply => ValueUseClassification::NotRewritableApply {
                reason: ApplyRewriteBlocker::EffectUnknown,
            },
            ValueProducerKind::Binding
            | ValueProducerKind::Block
            | ValueProducerKind::Branch
            | ValueProducerKind::Match
            | ValueProducerKind::Region => ValueUseClassification::UnusedValue,
        };
    }

    let copy_reads = value
        .uses
        .iter()
        .filter(|event| event.kind == UseKind::ReadCopy)
        .count();
    let moves = value
        .uses
        .iter()
        .filter(|event| event.kind == UseKind::Move)
        .count();
    let borrows = value
        .uses
        .iter()
        .filter(|event| matches!(event.kind, UseKind::BorrowShared | UseKind::BorrowMut))
        .count();
    let drops = value
        .uses
        .iter()
        .filter(|event| event.kind == UseKind::Drop)
        .count();

    if copy_reads == value.uses.len() {
        return ValueUseClassification::CopyOnly { reads: copy_reads };
    }

    if moves == 1 && copy_reads == 0 && borrows == 0 && drops == 0 {
        return ValueUseClassification::SingleMove;
    }

    ValueUseClassification::AffineConsumed {
        copy_reads,
        moves,
        borrows,
        drops,
    }
}

fn finding_for_value(value: &ValueUseSummary) -> Option<ValueUseFinding> {
    match value.classification {
        ValueUseClassification::CopyOnly { reads } => Some(ValueUseFinding::CopyOnly {
            value: value.value,
            reads,
        }),
        ValueUseClassification::SingleMove => Some(ValueUseFinding::SingleMove {
            value: value.value,
            repr: value.repr,
        }),
        ValueUseClassification::UnusedPureValue => Some(ValueUseFinding::UnusedPureValue {
            value: value.value,
            producer: value.producer,
        }),
        ValueUseClassification::NotRewritableApply { reason } => {
            Some(ValueUseFinding::NotRewritableApply {
                value: value.value,
                producer: value.producer,
                reason,
            })
        }
        ValueUseClassification::BodyResult
        | ValueUseClassification::AffineConsumed { .. }
        | ValueUseClassification::UnusedValue => None,
    }
}

fn forwarding_candidates_for_values(
    values: &[ValueUseSummary],
    applies_by_expr: &HashMap<ExprId, &super::ApplyIr>,
    bindings_by_id: &HashMap<BindingId, &CheckedBindingIr>,
) -> Vec<AffineForwardingCandidate> {
    let mut candidates = Vec::new();

    for value in values {
        if value.classification != ValueUseClassification::SingleMove {
            continue;
        }
        if value.producer_kind != ValueProducerKind::Binding {
            continue;
        }
        if !value.repr.is_runtime_reference() {
            continue;
        }
        let Some(binding) = value.binding else {
            continue;
        };
        let Some(binding_info) = bindings_by_id.get(&binding) else {
            continue;
        };
        if binding_info.mutable {
            continue;
        }

        let Some(event) = value.uses.first() else {
            continue;
        };
        let Some(apply) = applies_by_expr.get(&event.at) else {
            continue;
        };
        let Some(arg_index) = apply.args.iter().position(|arg| *arg == value.value) else {
            continue;
        };

        candidates.push(AffineForwardingCandidate {
            value: value.value,
            binding,
            binding_name: binding_info.name.clone(),
            producer: value.producer,
            apply_expr: event.at,
            apply_flavor: apply.flavor,
            arg_index,
            repr: value.repr,
            rewrite_blocker: ForwardingRewriteBlocker::StableBindingGraphRequired,
        });
    }

    candidates
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmMirModule {
    pub functions: Vec<WasmMirFunction>,
}

impl WasmMirModule {
    pub fn optimize(&mut self, level: OptimizationLevel) -> OptimizationReport {
        if level == OptimizationLevel::None {
            return OptimizationReport::default();
        }

        let mut report = OptimizationReport::default();

        for function in &mut self.functions {
            report.removed_nops += remove_nops(function);
            if level >= OptimizationLevel::Local {
                report.folded_constants += fold_i32_add_constants_to_fixpoint(function);
            }
        }

        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmMirFunction {
    pub name: String,
    pub instructions: Vec<WasmMirInstr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmMirInstr {
    Nop,
    I32Const(i32),
    I32Add,
    LocalGet(u32),
    LocalSet(u32),
    Drop,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OptimizationLevel {
    None,
    Hygiene,
    Local,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationReport {
    pub removed_nops: usize,
    pub folded_constants: usize,
}

fn remove_nops(function: &mut WasmMirFunction) -> usize {
    let before = function.instructions.len();
    function
        .instructions
        .retain(|instr| !matches!(instr, WasmMirInstr::Nop));
    before - function.instructions.len()
}

fn fold_i32_add_constants_to_fixpoint(function: &mut WasmMirFunction) -> usize {
    let mut folded = 0;

    loop {
        let pass_folded = fold_i32_add_constants(function);
        if pass_folded == 0 {
            return folded;
        }
        folded += pass_folded;
    }
}

fn fold_i32_add_constants(function: &mut WasmMirFunction) -> usize {
    let mut folded = 0;
    let mut output = Vec::with_capacity(function.instructions.len());
    let mut cursor = 0;

    while cursor < function.instructions.len() {
        if let [WasmMirInstr::I32Const(left), WasmMirInstr::I32Const(right), WasmMirInstr::I32Add] =
            &function.instructions[cursor..function.instructions.len().min(cursor + 3)]
        {
            output.push(WasmMirInstr::I32Const(left.wrapping_add(*right)));
            folded += 1;
            cursor += 3;
        } else {
            output.push(function.instructions[cursor].clone());
            cursor += 1;
        }
    }

    function.instructions = output;
    folded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Program;
    use crate::ir::builder::{build_checked_ir, CheckedProgramIr};
    use crate::ir::layout::SumVariantIdentity;
    use crate::ir::{HostAbi, InternalOnlyReason};
    use crate::parser::parse_program;
    use crate::type_checker::TypeChecker;

    fn parse_source(source: &str) -> Program {
        let (remaining, program) = parse_program(source).expect("source should parse");
        assert!(remaining.trim().is_empty(), "unparsed input: {remaining:?}");
        program
    }

    fn checked_ir(source: &str) -> CheckedProgramIr {
        let program = parse_source(source);
        checked_ir_for_program(&program)
    }

    fn checked_ir_for_program(program: &Program) -> CheckedProgramIr {
        let mut checker = TypeChecker::new();
        checker
            .check_program(program)
            .expect("source should type-check");
        build_checked_ir(program, &checker).expect("checked IR should build")
    }

    fn function_summary<'a>(
        summary: &'a ProgramValueUseSummary,
        name: &str,
    ) -> &'a FunctionValueUseSummary {
        summary
            .functions
            .iter()
            .find(|function| function.name == name)
            .expect("function summary should be present")
    }

    fn layout_function_summary<'a>(
        summary: &'a ProgramLayoutOptimizationSummary,
        name: &str,
    ) -> &'a FunctionLayoutOptimizationSummary {
        summary
            .functions
            .iter()
            .find(|function| function.name == name)
            .expect("function layout summary should be present")
    }

    fn sum_variant(tag: u32, name: &str) -> SumVariantIdentity {
        SumVariantIdentity {
            tag,
            name: name.to_string(),
        }
    }

    #[test]
    fn checked_ir_layout_optimization_summary_reports_sum_candidates() {
        let ir = checked_ir(
            r#"
fun keep_option_string: (value: Option<String>) -> Option<String> = {
    value
}

fun keep_result_scalar: (value: Result<Int32, Boolean>) -> Result<Int32, Boolean> = {
    value
}

fun keep_range: (value: Range<Int32>) -> Range<Int32> = {
    value
}
"#,
        );

        let summary = summarize_checked_program_layout_optimizations(&ir);

        let option = layout_function_summary(&summary, "keep_option_string");
        assert_eq!(option.sum_candidates.len(), 1);
        let option_candidate = &option.sum_candidates[0];
        assert_eq!(option_candidate.strategy, SumStrategy::TaggedPayload);
        assert_eq!(
            option_candidate.rewrite_blocker,
            LayoutOptimizationRewriteBlocker::AdvisoryOnly
        );
        assert_eq!(
            option_candidate.candidates,
            vec![SumOptimizationCandidate::NullNiche {
                payload_variant: sum_variant(1, "Some"),
            }]
        );

        let result = layout_function_summary(&summary, "keep_result_scalar");
        assert_eq!(result.sum_candidates.len(), 1);
        let result_candidate = &result.sum_candidates[0];
        assert_eq!(result_candidate.strategy, SumStrategy::TaggedPayload);
        assert_eq!(
            result_candidate.rewrite_blocker,
            LayoutOptimizationRewriteBlocker::AdvisoryOnly
        );
        assert!(result_candidate
            .candidates
            .contains(&SumOptimizationCandidate::ScalarPair {
                payload_variants: vec![sum_variant(0, "Err"), sum_variant(1, "Ok")]
            }));
        assert!(result_candidate
            .candidates
            .contains(&SumOptimizationCandidate::ScalarLocal {
                payload_variants: vec![sum_variant(0, "Err"), sum_variant(1, "Ok")]
            }));

        let range = layout_function_summary(&summary, "keep_range");
        assert!(range.sum_candidates.is_empty());
    }

    #[test]
    fn checked_ir_layout_optimization_summary_keeps_host_abi_internal_only() {
        let ir = checked_ir(
            r#"
fun keep_option: (value: Option<Int32>) -> Option<Int32> = {
    value
}
"#,
        );

        let summary = summarize_checked_program_layout_optimizations(&ir);
        let function = ir
            .functions
            .iter()
            .find(|function| function.name == "keep_option")
            .expect("function should be present");
        let layout_summary = layout_function_summary(&summary, "keep_option");
        let composite = InternalOnlyReason::CompositeHostAbiUnstable;

        assert_eq!(layout_summary.sum_candidates.len(), 1);
        assert_eq!(
            function.lowering.param_host_abis,
            vec![HostAbi::InternalOnly(composite.clone())]
        );
        assert_eq!(
            function.lowering.return_host_abi,
            HostAbi::InternalOnly(composite)
        );
        assert!(!function.lowering.readiness.v001_host_abi_eligible);
    }

    #[test]
    fn checked_ir_layout_optimization_summary_does_not_mutate_ir_or_wat() {
        let program = parse_source(
            r#"
fun keep_option: (value: Option<Int32>) -> Option<Int32> = {
    value
}

fun main: () -> Int32 = {
    42
}
"#,
        );
        let before_wat = crate::generate(&program).expect("source should generate WAT");
        let ir = checked_ir_for_program(&program);
        let before_ir = ir.clone();

        let summary = summarize_checked_program_layout_optimizations(&ir);
        assert_eq!(
            layout_function_summary(&summary, "keep_option")
                .sum_candidates
                .len(),
            1
        );
        assert_eq!(ir, before_ir);
        assert_eq!(
            crate::generate(&program).expect("source should still generate WAT"),
            before_wat
        );
    }

    #[test]
    fn checked_ir_value_use_summary_marks_scalar_copy_reads() {
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

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");
        let copied_literals = main
            .values
            .iter()
            .filter(|value| value.producer_kind == ValueProducerKind::Literal)
            .filter(|value| {
                matches!(
                    value.classification,
                    ValueUseClassification::CopyOnly { reads: 1 }
                )
            })
            .count();

        assert_eq!(copied_literals, 2);
        assert_eq!(
            main.findings
                .iter()
                .filter(|finding| matches!(finding, ValueUseFinding::CopyOnly { reads: 1, .. }))
                .count(),
            2
        );
    }

    #[test]
    fn checked_ir_value_use_summary_marks_composite_single_move() {
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

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");
        let moved_list = main
            .values
            .iter()
            .find(|value| {
                value.producer_kind == ValueProducerKind::Literal
                    && value.repr.is_runtime_reference()
            })
            .expect("list literal should be tracked");

        assert_eq!(
            moved_list.classification,
            ValueUseClassification::SingleMove
        );
        assert!(main.findings.iter().any(|finding| {
            matches!(
                finding,
                ValueUseFinding::SingleMove { value, repr }
                    if *value == moved_list.value && *repr == moved_list.repr
            )
        }));
    }

    #[test]
    fn checked_ir_value_use_summary_reports_affine_forwarding_candidate() {
        let ir = checked_ir(
            r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: (items: List<Int32>) -> List<Int32> = {
    items |> keep
}
"#,
        );

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");

        assert_eq!(main.forwarding_candidates.len(), 1);
        let candidate = &main.forwarding_candidates[0];
        let value = main
            .values
            .iter()
            .find(|value| value.value == candidate.value)
            .expect("candidate value should be summarized");

        assert_eq!(value.producer_kind, ValueProducerKind::Binding);
        assert_eq!(candidate.binding_name, "items");
        assert_eq!(value.binding, Some(candidate.binding));
        assert_eq!(value.classification, ValueUseClassification::SingleMove);
        assert_eq!(candidate.apply_flavor, ApplyFlavor::Pipe);
        assert_eq!(candidate.arg_index, 0);
        assert_eq!(
            candidate.rewrite_blocker,
            ForwardingRewriteBlocker::StableBindingGraphRequired
        );
        assert!(candidate.repr.is_runtime_reference());
    }

    #[test]
    fn checked_ir_value_use_summary_reports_local_alias_forwarding_candidate() {
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

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");

        assert_eq!(main.forwarding_candidates.len(), 1);
        let candidate = &main.forwarding_candidates[0];
        let value = main
            .values
            .iter()
            .find(|value| value.value == candidate.value)
            .expect("candidate value should be summarized");

        assert_eq!(candidate.binding_name, "alias");
        assert_eq!(value.binding, Some(candidate.binding));
        assert_eq!(value.producer_kind, ValueProducerKind::Binding);
        assert_eq!(candidate.apply_flavor, ApplyFlavor::Pipe);
        assert_eq!(
            candidate.rewrite_blocker,
            ForwardingRewriteBlocker::StableBindingGraphRequired
        );
    }

    #[test]
    fn checked_ir_value_use_summary_does_not_forward_mutable_alias() {
        let ir = checked_ir(
            r#"
fun keep: (items: List<Int32>) -> List<Int32> = {
    items
}

fun main: () -> List<Int32> = {
    mut val alias: List<Int32> = []
    alias |> keep
}
"#,
        );

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");

        assert!(main.forwarding_candidates.is_empty());
        assert!(main.values.iter().any(|value| {
            value.binding.is_some() && value.classification == ValueUseClassification::SingleMove
        }));
    }

    #[test]
    fn checked_ir_value_use_summary_does_not_forward_literal_direct_move() {
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

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");

        assert!(main.forwarding_candidates.is_empty());
        assert!(main.findings.iter().any(|finding| {
            matches!(
                finding,
                ValueUseFinding::SingleMove { repr, .. } if repr.is_runtime_reference()
            )
        }));
    }

    #[test]
    fn checked_ir_value_use_summary_does_not_forward_scalar_copy() {
        let ir = checked_ir(
            r#"
fun inc: (value: Int32) -> Int32 = {
    value + 1
}

fun main: (value: Int32) -> Int32 = {
    value |> inc
}
"#,
        );

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");

        assert!(main.forwarding_candidates.is_empty());
        assert!(main
            .findings
            .iter()
            .any(|finding| { matches!(finding, ValueUseFinding::CopyOnly { reads: 1, .. }) }));
    }

    #[test]
    fn checked_ir_value_use_summary_treats_body_result_as_live() {
        let ir = checked_ir(
            r#"
fun main: () -> Int32 = {
    42
}
"#,
        );

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");

        assert_eq!(main.values.len(), 1);
        assert_eq!(
            main.values[0].classification,
            ValueUseClassification::BodyResult
        );
        assert!(main.findings.is_empty());
    }

    #[test]
    fn checked_ir_value_use_summary_keeps_unused_apply_non_rewritable() {
        let ir = checked_ir(
            r#"
fun inc: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    (1) inc
    42
}
"#,
        );

        let program_summary = summarize_checked_program_value_uses(&ir);
        let main = function_summary(&program_summary, "main");
        let apply_result = main
            .values
            .iter()
            .find(|value| value.producer_kind == ValueProducerKind::Apply)
            .expect("unused apply result should be tracked");

        assert_eq!(
            apply_result.classification,
            ValueUseClassification::NotRewritableApply {
                reason: ApplyRewriteBlocker::EffectUnknown
            }
        );
        assert!(main.findings.iter().any(|finding| {
            matches!(
                finding,
                ValueUseFinding::NotRewritableApply {
                    value,
                    reason: ApplyRewriteBlocker::EffectUnknown,
                    ..
                } if *value == apply_result.value
            )
        }));
    }

    #[test]
    fn hygiene_optimization_removes_nops() {
        let mut module = WasmMirModule {
            functions: vec![WasmMirFunction {
                name: "score".to_string(),
                instructions: vec![
                    WasmMirInstr::Nop,
                    WasmMirInstr::I32Const(1),
                    WasmMirInstr::Nop,
                    WasmMirInstr::Return,
                ],
            }],
        };

        let report = module.optimize(OptimizationLevel::Hygiene);
        assert_eq!(report.removed_nops, 2);
        assert_eq!(
            module.functions[0].instructions,
            vec![WasmMirInstr::I32Const(1), WasmMirInstr::Return]
        );
    }

    #[test]
    fn local_optimization_folds_adjacent_i32_add() {
        let mut module = WasmMirModule {
            functions: vec![WasmMirFunction {
                name: "score".to_string(),
                instructions: vec![
                    WasmMirInstr::I32Const(40),
                    WasmMirInstr::I32Const(2),
                    WasmMirInstr::I32Add,
                    WasmMirInstr::Return,
                ],
            }],
        };

        let report = module.optimize(OptimizationLevel::Local);
        assert_eq!(report.folded_constants, 1);
        assert_eq!(
            module.functions[0].instructions,
            vec![WasmMirInstr::I32Const(42), WasmMirInstr::Return]
        );
    }

    #[test]
    fn local_optimization_folds_i32_add_chains_to_fixpoint() {
        let mut module = WasmMirModule {
            functions: vec![WasmMirFunction {
                name: "score".to_string(),
                instructions: vec![
                    WasmMirInstr::I32Const(1),
                    WasmMirInstr::I32Const(2),
                    WasmMirInstr::I32Add,
                    WasmMirInstr::I32Const(3),
                    WasmMirInstr::I32Add,
                    WasmMirInstr::Return,
                ],
            }],
        };

        let report = module.optimize(OptimizationLevel::Local);
        assert_eq!(report.folded_constants, 2);
        assert_eq!(
            module.functions[0].instructions,
            vec![WasmMirInstr::I32Const(6), WasmMirInstr::Return]
        );
    }

    #[test]
    fn none_optimization_is_noop() {
        let mut module = WasmMirModule {
            functions: vec![WasmMirFunction {
                name: "score".to_string(),
                instructions: vec![
                    WasmMirInstr::Nop,
                    WasmMirInstr::I32Const(1),
                    WasmMirInstr::I32Const(2),
                    WasmMirInstr::I32Add,
                ],
            }],
        };
        let original = module.clone();

        let report = module.optimize(OptimizationLevel::None);
        assert_eq!(report, OptimizationReport::default());
        assert_eq!(module, original);
    }

    #[test]
    fn local_optimization_uses_i32_wrapping_semantics() {
        let mut module = WasmMirModule {
            functions: vec![WasmMirFunction {
                name: "wrap".to_string(),
                instructions: vec![
                    WasmMirInstr::I32Const(i32::MAX),
                    WasmMirInstr::I32Const(1),
                    WasmMirInstr::I32Add,
                ],
            }],
        };

        let report = module.optimize(OptimizationLevel::Local);
        assert_eq!(report.folded_constants, 1);
        assert_eq!(
            module.functions[0].instructions,
            vec![WasmMirInstr::I32Const(i32::MIN)]
        );
    }
}
