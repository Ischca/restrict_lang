//! Low-level optimization stage for the future Wasm MIR pipeline.

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
                report.folded_constants += fold_i32_add_constants(function);
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
