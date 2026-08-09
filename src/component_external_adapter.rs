//! Guest-import Canonical ABI adapters for externally authoritative WIT.
//!
//! This is intentionally separate from the Restrict-owned guest-export
//! adapter. It replaces generated type-checking stubs with internal-ABI
//! wrappers that synchronously lower into exact Standard32 imports and lift
//! returned values back into Restrict's private arena representation.

use super::{ExternalWitContract, ExternalWitRestrictImportBinding, ResolvedExportImplementation};
use crate::component::{ComponentNamedTypeIr, ComponentNamedTypeKind, ComponentValueType};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;
use wit_bindgen_core::abi::{
    self, AbiVariant, Bindgen, Bitcast, Instruction, LiftLower, WasmSignature, WasmType,
};
use wit_bindgen_core::wit_parser::{
    Alignment, ArchitectureSize, Function, ManglingAndAbi, Resolve, SizeAlign, Type, TypeDefKind,
    WasmImport, WorldId, WorldItem, WorldKey,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub(super) enum ExternalImportAdapterError {
    #[error("normalized external WIT could not be resolved for guest-import adapters: {0}")]
    WitResolution(String),
    #[error("external WIT world '{0}' was not found for guest-import adapters")]
    MissingWorld(String),
    #[error("external WIT import '{module}#{name}' has no deterministic Restrict binding")]
    MissingBinding { module: String, name: String },
    #[error("Restrict binding '{0}' was not found in normalized external WIT")]
    MissingWitImport(String),
    #[error("generated Core WAT has no function declaration for external-WIT stub '{0}'")]
    MissingStub(String),
    #[error("generated Core WAT declares external-WIT stub '{0}' more than once")]
    DuplicateStub(String),
    #[error("external-WIT export '{0}' still requires a guest-export rich Canonical ABI adapter")]
    RichExportUnsupported(String),
    #[error("Canonical ABI guest-import adapter for '{0}' is unsupported: {1}")]
    UnsupportedCanonicalAdapter(String, String),
    #[error("generated Core WAT has no final module delimiter")]
    MissingModuleDelimiter,
}

pub(super) struct AdaptedExternalCore {
    pub wat: String,
    pub export_symbols: BTreeMap<String, String>,
}

pub(super) fn generate_external_import_adapter_core_wat(
    core_wat: &str,
    contract: &ExternalWitContract,
    exports: &[ResolvedExportImplementation],
    invocation_arena_addr: u32,
) -> Result<AdaptedExternalCore, ExternalImportAdapterError> {
    let mut resolve = Resolve::default();
    let package = resolve
        .push_str("restrict-external-adapter.wit", &contract.adapter_wit)
        .map_err(|error| ExternalImportAdapterError::WitResolution(error.to_string()))?;
    let world = resolve
        .select_world(&[package], Some(&contract.binding_ir.world))
        .map_err(|_| ExternalImportAdapterError::MissingWorld(contract.binding_ir.world.clone()))?;
    let mut sizes = SizeAlign::default();
    sizes.fill(&resolve);

    let mut bindings = contract
        .binding_plan
        .imports
        .iter()
        .map(|binding| {
            (
                (binding.core_module.clone(), binding.core_name.clone()),
                binding,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut raw_imports = String::new();
    raw_imports.push_str("\n  ;; external-WIT Standard32 guest imports\n");
    let mut generated = String::new();
    let mut adapted = core_wat.to_string();
    let mut import_index = 0usize;
    visit_world_import_functions(&resolve, world, |interface, function| {
        let (module, name) = resolve.wasm_import_name(
            ManglingAndAbi::Standard32,
            WasmImport::Func {
                interface,
                func: function,
            },
        );
        let binding = bindings
            .remove(&(module.clone(), name.clone()))
            .ok_or_else(|| ExternalImportAdapterError::MissingBinding {
                module: module.clone(),
                name: name.clone(),
            })?;
        let signature = resolve.wasm_signature(AbiVariant::GuestImport, function);
        let raw_symbol = format!("__restrict_external_canonical_import_{import_index}");
        emit_raw_import(&mut raw_imports, &module, &name, &raw_symbol, &signature);
        let context = ImportAdapterContext {
            binding,
            function,
            signature: &signature,
            raw_symbol: &raw_symbol,
            named_types: &contract.binding_plan.types,
        };
        let mut bindgen = ImportWatAbiBindgen::new(&resolve, &sizes, context);
        abi::call(
            &resolve,
            AbiVariant::GuestImport,
            LiftLower::LowerArgsLiftResults,
            function,
            &mut bindgen,
            false,
        );
        generated.push_str(&bindgen.finish()?);
        adapted = rename_stub_declaration(
            &adapted,
            &binding.local_function,
            &format!("__restrict_external_typecheck_stub_{import_index}"),
        )?;
        import_index += 1;
        Ok(())
    })?;
    if let Some((_, binding)) = bindings.into_iter().next() {
        return Err(ExternalImportAdapterError::MissingWitImport(
            binding.canonical_identity.clone(),
        ));
    }

    if !contract.binding_plan.imports.is_empty() && !adapted.contains("(export \"cm32p2_memory\"") {
        emit_canonical_realloc(&mut generated);
        generated.push_str("  (export \"cm32p2_memory\" (memory 0))\n");
    }
    let export_symbols = emit_scalar_export_entry_wrappers(
        &mut generated,
        contract,
        exports,
        invocation_arena_addr,
    )?;

    let module_header_end = adapted
        .find('\n')
        .ok_or(ExternalImportAdapterError::MissingModuleDelimiter)?
        + 1;
    adapted.insert_str(module_header_end, &raw_imports);
    let module_end = adapted
        .rfind(')')
        .ok_or(ExternalImportAdapterError::MissingModuleDelimiter)?;
    adapted.insert_str(module_end, &generated);
    Ok(AdaptedExternalCore {
        wat: adapted,
        export_symbols,
    })
}

fn emit_canonical_realloc(output: &mut String) {
    output.push_str(
        "  (func $__restrict_external_cabi_realloc (param $old_ptr i32) (param $old_size i32) (param $align i32) (param $new_size i32) (result i32)\n",
    );
    output.push_str("    (local $allocation i32)\n");
    output.push_str("    (local $copy_size i32)\n");
    output.push_str("    local.get $new_size\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if (result i32)\n");
    output.push_str("      (then i32.const 0)\n");
    output.push_str("      (else\n");
    output.push_str("        local.get $align\n");
    output.push_str("        i32.eqz\n");
    output.push_str("        local.get $align\n");
    output.push_str("        i32.const 1\n");
    output.push_str("        i32.sub\n");
    output.push_str("        local.get $align\n");
    output.push_str("        i32.and\n");
    output.push_str("        i32.const 0\n");
    output.push_str("        i32.ne\n");
    output.push_str("        i32.or\n");
    output.push_str("        (if (then unreachable))\n");
    output.push_str("        local.get $new_size\n");
    output.push_str("        local.get $align\n");
    output.push_str("        i32.const 1\n");
    output.push_str("        i32.sub\n");
    output.push_str("        i32.add\n");
    output.push_str("        call $allocate\n");
    output.push_str("        local.get $align\n");
    output.push_str("        i32.const 1\n");
    output.push_str("        i32.sub\n");
    output.push_str("        i32.add\n");
    output.push_str("        local.get $align\n");
    output.push_str("        i32.const 1\n");
    output.push_str("        i32.sub\n");
    output.push_str("        i32.const -1\n");
    output.push_str("        i32.xor\n");
    output.push_str("        i32.and\n");
    output.push_str("        local.set $allocation\n");
    output.push_str("        local.get $old_ptr\n");
    output.push_str("        i32.eqz\n");
    output.push_str("        i32.eqz\n");
    output.push_str("        (if\n");
    output.push_str("          (then\n");
    output.push_str("            local.get $old_size\n");
    output.push_str("            local.get $new_size\n");
    output.push_str("            i32.lt_u\n");
    output.push_str("            (if (result i32)\n");
    output.push_str("              (then local.get $old_size)\n");
    output.push_str("              (else local.get $new_size)\n");
    output.push_str("            )\n");
    output.push_str("            local.set $copy_size\n");
    output.push_str("            local.get $allocation\n");
    output.push_str("            local.get $old_ptr\n");
    output.push_str("            local.get $copy_size\n");
    output.push_str("            memory.copy\n");
    output.push_str("          )\n");
    output.push_str("        )\n");
    output.push_str("        local.get $allocation\n");
    output.push_str("      )\n");
    output.push_str("    )\n");
    output.push_str("  )\n");
    output.push_str("  (export \"cabi_realloc\" (func $__restrict_external_cabi_realloc))\n");
    output.push_str("  (export \"cm32p2_realloc\" (func $__restrict_external_cabi_realloc))\n");
}

fn visit_world_import_functions<'a>(
    resolve: &'a Resolve,
    world: WorldId,
    mut visit: impl FnMut(Option<&'a WorldKey>, &'a Function) -> Result<(), ExternalImportAdapterError>,
) -> Result<(), ExternalImportAdapterError> {
    for (world_key, item) in &resolve.worlds[world].imports {
        match item {
            WorldItem::Function(function) => visit(None, function)?,
            WorldItem::Interface { id, .. } => {
                for function in resolve.interfaces[*id].functions.values() {
                    visit(Some(world_key), function)?;
                }
            }
            WorldItem::Type { .. } => {}
        }
    }
    Ok(())
}

fn emit_raw_import(
    output: &mut String,
    module: &str,
    name: &str,
    symbol: &str,
    signature: &WasmSignature,
) {
    output.push_str("  (import \"");
    append_wat_string_contents(output, module);
    output.push_str("\" \"");
    append_wat_string_contents(output, name);
    output.push_str(&format!("\" (func ${symbol}"));
    for ty in &signature.params {
        output.push_str(&format!(" (param {})", wat_type(*ty)));
    }
    for ty in &signature.results {
        output.push_str(&format!(" (result {})", wat_type(*ty)));
    }
    output.push_str("))\n");
}

fn rename_stub_declaration(
    wat: &str,
    source_symbol: &str,
    placeholder_symbol: &str,
) -> Result<String, ExternalImportAdapterError> {
    let needle = format!("(func ${source_symbol}");
    let matches = wat.match_indices(&needle).collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(ExternalImportAdapterError::MissingStub(
            source_symbol.to_string(),
        )),
        [(offset, _)] => {
            let name_start = offset + "(func $".len();
            let name_end = name_start + source_symbol.len();
            let mut renamed = String::with_capacity(
                wat.len() + placeholder_symbol.len().saturating_sub(source_symbol.len()),
            );
            renamed.push_str(&wat[..name_start]);
            renamed.push_str(placeholder_symbol);
            renamed.push_str(&wat[name_end..]);
            Ok(renamed)
        }
        _ => Err(ExternalImportAdapterError::DuplicateStub(
            source_symbol.to_string(),
        )),
    }
}

fn emit_scalar_export_entry_wrappers(
    output: &mut String,
    contract: &ExternalWitContract,
    exports: &[ResolvedExportImplementation],
    invocation_arena_addr: u32,
) -> Result<BTreeMap<String, String>, ExternalImportAdapterError> {
    if exports.is_empty() {
        return Ok(BTreeMap::new());
    }
    output.push_str("\n  ;; external-WIT invocation lifetime wrappers\n");
    output.push_str("  (global $__restrict_external_adapter_depth (mut i32) (i32.const 0))\n");
    let obligations = contract
        .binding_plan
        .exports
        .iter()
        .map(|obligation| (obligation.canonical_core_name.as_str(), obligation))
        .collect::<BTreeMap<_, _>>();
    let mut symbols = BTreeMap::new();
    for (index, implementation) in exports.iter().enumerate() {
        let obligation = obligations
            .get(implementation.canonical_core_name.as_str())
            .expect("validated export implementation must retain its obligation");
        let rich = obligation
            .params
            .iter()
            .any(|param| component_value_is_pointer(&param.ty))
            || obligation
                .result
                .as_ref()
                .is_some_and(component_value_is_pointer);
        if rich {
            return Err(ExternalImportAdapterError::RichExportUnsupported(
                obligation.canonical_identity.clone(),
            ));
        }
        let wrapper = format!("__restrict_external_export_entry_{index}");
        output.push_str(&format!("  (func ${wrapper}"));
        for (param_index, param) in obligation.params.iter().enumerate() {
            output.push_str(&format!(
                " (param $external_param_{param_index} {})",
                component_internal_core_type(&param.ty).wat()
            ));
        }
        if let Some(result) = &obligation.result {
            output.push_str(&format!(
                " (result {})",
                component_internal_core_type(result).wat()
            ));
        }
        output.push('\n');
        output.push_str("    (local $external_prev_arena i32)\n");
        output.push_str("    (local $external_entry_depth i32)\n");
        output.push_str("    (local $external_arena_mark i32)\n");
        if let Some(result) = &obligation.result {
            output.push_str(&format!(
                "    (local $external_result {})\n",
                component_internal_core_type(result).wat()
            ));
        }
        emit_invocation_enter(output, invocation_arena_addr);
        for (param_index, _) in obligation.params.iter().enumerate() {
            output.push_str(&format!("    local.get $external_param_{param_index}\n"));
        }
        output.push_str(&format!("    call ${}\n", implementation.source_function));
        if obligation.result.is_some() {
            output.push_str("    local.set $external_result\n");
        }
        emit_invocation_exit(output, invocation_arena_addr);
        if obligation.result.is_some() {
            output.push_str("    local.get $external_result\n");
        }
        output.push_str("  )\n");
        output.push_str(&format!("  (export \"{wrapper}\" (func ${wrapper}))\n"));
        symbols.insert(implementation.source_function.clone(), wrapper);
    }
    Ok(symbols)
}

fn emit_invocation_enter(output: &mut String, arena_addr: u32) {
    output.push_str("    global.get $current_arena\n");
    output.push_str("    local.set $external_prev_arena\n");
    output.push_str("    global.get $__restrict_external_adapter_depth\n");
    output.push_str("    local.tee $external_entry_depth\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if\n");
    output.push_str("      (then\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        call $arena_init\n");
    output.push_str("        drop\n");
    output.push_str("      )\n");
    output.push_str("      (else\n");
    output.push_str(&format!("        i32.const {}\n", arena_addr + 4));
    output.push_str("        i32.load\n");
    output.push_str("        local.set $external_arena_mark\n");
    output.push_str("      )\n");
    output.push_str("    )\n");
    output.push_str("    local.get $external_entry_depth\n");
    output.push_str("    i32.const 1\n");
    output.push_str("    i32.add\n");
    output.push_str("    global.set $__restrict_external_adapter_depth\n");
    output.push_str(&format!("    i32.const {arena_addr}\n"));
    output.push_str("    global.set $current_arena\n");
}

fn emit_invocation_exit(output: &mut String, arena_addr: u32) {
    output.push_str("    local.get $external_entry_depth\n");
    output.push_str("    global.set $__restrict_external_adapter_depth\n");
    output.push_str("    local.get $external_entry_depth\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if\n");
    output.push_str("      (then\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        call $arena_reset\n");
    output.push_str("      )\n");
    output.push_str("      (else\n");
    output.push_str(&format!("        i32.const {}\n", arena_addr + 4));
    output.push_str("        local.get $external_arena_mark\n");
    output.push_str("        i32.store\n");
    output.push_str("      )\n");
    output.push_str("    )\n");
    output.push_str("    local.get $external_prev_arena\n");
    output.push_str("    global.set $current_arena\n");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreType {
    I32,
    I64,
    F32,
    F64,
}

impl CoreType {
    fn wat(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    fn bytes(self) -> usize {
        match self {
            Self::I32 | Self::F32 => 4,
            Self::I64 | Self::F64 => 8,
        }
    }
}

impl From<WasmType> for CoreType {
    fn from(value: WasmType) -> Self {
        match value {
            WasmType::I32 | WasmType::Pointer | WasmType::Length => Self::I32,
            WasmType::I64 | WasmType::PointerOrI64 => Self::I64,
            WasmType::F32 => Self::F32,
            WasmType::F64 => Self::F64,
        }
    }
}

#[derive(Debug, Clone)]
struct Operand {
    source: OperandSource,
    ty: Option<CoreType>,
}

#[derive(Debug, Clone)]
enum OperandSource {
    Local(String),
    Special(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpecialKind {
    VariantPayload,
    IterElement,
    IterBase,
}

#[derive(Debug)]
struct FinishedBlock {
    code: String,
    results: Vec<Operand>,
    specials: Vec<usize>,
}

struct ImportAdapterContext<'a> {
    binding: &'a ExternalWitRestrictImportBinding,
    function: &'a Function,
    signature: &'a WasmSignature,
    raw_symbol: &'a str,
    named_types: &'a [ComponentNamedTypeIr],
}

struct ImportWatAbiBindgen<'a> {
    resolve: &'a Resolve,
    sizes: &'a SizeAlign,
    context: ImportAdapterContext<'a>,
    code: String,
    code_stack: Vec<String>,
    block_specials_stack: Vec<Vec<usize>>,
    blocks: Vec<FinishedBlock>,
    locals: BTreeMap<String, CoreType>,
    special_types: BTreeMap<usize, CoreType>,
    special_kinds: BTreeMap<usize, SpecialKind>,
    lift_array_lengths: VecDeque<Option<usize>>,
    lower_array_lengths: VecDeque<Option<usize>>,
    next_local: usize,
    next_special: usize,
    returned: Vec<Operand>,
    error: Option<String>,
}

impl<'a> ImportWatAbiBindgen<'a> {
    fn new(resolve: &'a Resolve, sizes: &'a SizeAlign, context: ImportAdapterContext<'a>) -> Self {
        let named_types = context
            .named_types
            .iter()
            .map(|ty| (ty.name.as_str(), ty))
            .collect::<BTreeMap<_, _>>();
        let mut lift_array_lengths = VecDeque::new();
        let mut lower_array_lengths = VecDeque::new();
        for param in &context.binding.params {
            collect_array_constraints(
                &param.ty,
                &named_types,
                &mut BTreeSet::new(),
                &mut lower_array_lengths,
            );
        }
        if let Some(result) = &context.binding.result {
            collect_array_constraints(
                result,
                &named_types,
                &mut BTreeSet::new(),
                &mut lift_array_lengths,
            );
        }
        Self {
            resolve,
            sizes,
            context,
            code: String::new(),
            code_stack: Vec::new(),
            block_specials_stack: Vec::new(),
            blocks: Vec::new(),
            locals: BTreeMap::new(),
            special_types: BTreeMap::new(),
            special_kinds: BTreeMap::new(),
            lift_array_lengths,
            lower_array_lengths,
            next_local: 0,
            next_special: 0,
            returned: Vec::new(),
            error: None,
        }
    }

    fn fail(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(message.into());
        }
    }

    fn temp(&mut self, ty: CoreType) -> Operand {
        let name = format!("__restrict_external_cabi_tmp_{}", self.next_local);
        self.next_local += 1;
        self.locals.insert(name.clone(), ty);
        Operand {
            source: OperandSource::Local(name),
            ty: Some(ty),
        }
    }

    fn special(&mut self, kind: SpecialKind, ty: Option<CoreType>) -> Operand {
        let id = self.next_special;
        self.next_special += 1;
        self.special_kinds.insert(id, kind);
        if let Some(ty) = ty {
            self.special_types.insert(id, ty);
        }
        if let Some(specials) = self.block_specials_stack.last_mut() {
            specials.push(id);
        }
        Operand {
            source: OperandSource::Special(id),
            ty,
        }
    }

    fn operand_name(&mut self, operand: &Operand, expected: CoreType) -> String {
        match &operand.source {
            OperandSource::Local(name) => {
                if operand.ty.is_some_and(|actual| actual != expected) {
                    self.fail(format!(
                        "internal adapter operand type mismatch: expected {}, found {}",
                        expected.wat(),
                        operand.ty.expect("checked above").wat()
                    ));
                }
                name.clone()
            }
            OperandSource::Special(id) => {
                if let Some(actual) = self.special_types.get(id) {
                    if *actual != expected {
                        self.fail(format!(
                            "internal adapter special operand type mismatch: expected {}, found {}",
                            expected.wat(),
                            actual.wat()
                        ));
                    }
                } else {
                    self.special_types.insert(*id, expected);
                }
                format!("__restrict_external_cabi_special_{id}")
            }
        }
    }

    fn emit_get(&mut self, operand: &Operand, expected: CoreType) {
        let name = self.operand_name(operand, expected);
        self.code.push_str(&format!("    local.get ${name}\n"));
    }

    fn emit_set(&mut self, operand: &Operand, expected: CoreType) {
        let name = self.operand_name(operand, expected);
        self.code.push_str(&format!("    local.set ${name}\n"));
    }

    fn constant(&mut self, ty: CoreType, instruction: &str) -> Operand {
        let result = self.temp(ty);
        self.code.push_str(instruction);
        self.emit_set(&result, ty);
        result
    }

    fn zero(&mut self, ty: CoreType) -> Operand {
        self.constant(ty, &format!("    {}.const 0\n", ty.wat()))
    }

    fn coerce_noop(&mut self, mut operand: Operand, ty: CoreType) -> Operand {
        let _ = self.operand_name(&operand, ty);
        operand.ty = Some(ty);
        operand
    }

    fn emit_bitcast(&mut self, operand: &Operand, cast: &Bitcast) -> Operand {
        match cast {
            Bitcast::None => operand.clone(),
            Bitcast::Sequence(sequence) => {
                let [first, second] = &**sequence;
                let intermediate = self.emit_bitcast(operand, first);
                self.emit_bitcast(&intermediate, second)
            }
            Bitcast::F32ToI32 => {
                self.cast_instruction(operand, CoreType::F32, CoreType::I32, "i32.reinterpret_f32")
            }
            Bitcast::F64ToI64 => {
                self.cast_instruction(operand, CoreType::F64, CoreType::I64, "i64.reinterpret_f64")
            }
            Bitcast::I32ToI64 => {
                self.cast_instruction(operand, CoreType::I32, CoreType::I64, "i64.extend_i32_s")
            }
            Bitcast::F32ToI64 => {
                let bits = self.emit_bitcast(operand, &Bitcast::F32ToI32);
                self.cast_instruction(&bits, CoreType::I32, CoreType::I64, "i64.extend_i32_u")
            }
            Bitcast::I32ToF32 => {
                self.cast_instruction(operand, CoreType::I32, CoreType::F32, "f32.reinterpret_i32")
            }
            Bitcast::I64ToF64 => {
                self.cast_instruction(operand, CoreType::I64, CoreType::F64, "f64.reinterpret_i64")
            }
            Bitcast::I64ToI32 => {
                self.cast_instruction(operand, CoreType::I64, CoreType::I32, "i32.wrap_i64")
            }
            Bitcast::I64ToF32 => {
                let low = self.emit_bitcast(operand, &Bitcast::I64ToI32);
                self.emit_bitcast(&low, &Bitcast::I32ToF32)
            }
            Bitcast::P64ToI64 | Bitcast::I64ToP64 => {
                self.coerce_noop(operand.clone(), CoreType::I64)
            }
            Bitcast::P64ToP => {
                self.cast_instruction(operand, CoreType::I64, CoreType::I32, "i32.wrap_i64")
            }
            Bitcast::PToP64 => {
                self.cast_instruction(operand, CoreType::I32, CoreType::I64, "i64.extend_i32_u")
            }
            Bitcast::I32ToP
            | Bitcast::PToI32
            | Bitcast::PToL
            | Bitcast::LToP
            | Bitcast::I32ToL
            | Bitcast::LToI32 => self.coerce_noop(operand.clone(), CoreType::I32),
            Bitcast::I64ToL => self.emit_bitcast(operand, &Bitcast::I64ToI32),
            Bitcast::LToI64 => {
                self.cast_instruction(operand, CoreType::I32, CoreType::I64, "i64.extend_i32_u")
            }
        }
    }

    fn cast_instruction(
        &mut self,
        operand: &Operand,
        from: CoreType,
        to: CoreType,
        instruction: &str,
    ) -> Operand {
        let result = self.temp(to);
        self.emit_get(operand, from);
        self.code.push_str(&format!("    {instruction}\n"));
        self.emit_set(&result, to);
        result
    }

    fn emit_load(
        &mut self,
        address: &Operand,
        offset: ArchitectureSize,
        ty: CoreType,
        instruction: &str,
    ) -> Operand {
        let result = self.temp(ty);
        self.emit_get(address, CoreType::I32);
        self.code.push_str(&format!(
            "    {instruction} offset={}\n",
            offset.size_wasm32()
        ));
        self.emit_set(&result, ty);
        result
    }

    fn emit_store(
        &mut self,
        value: &Operand,
        address: &Operand,
        offset: ArchitectureSize,
        ty: CoreType,
        instruction: &str,
    ) {
        self.emit_get(address, CoreType::I32);
        self.emit_get(value, ty);
        self.code.push_str(&format!(
            "    {instruction} offset={}\n",
            offset.size_wasm32()
        ));
    }

    fn pop_blocks(&mut self, count: usize) -> Vec<FinishedBlock> {
        if self.blocks.len() < count {
            self.fail(format!(
                "Canonical ABI generator requested {count} blocks with only {} available",
                self.blocks.len()
            ));
            return Vec::new();
        }
        self.blocks.split_off(self.blocks.len() - count)
    }

    fn internal_type(&mut self, ty: &Type) -> CoreType {
        internal_core_type(self.resolve, ty).unwrap_or_else(|message| {
            self.fail(message);
            CoreType::I32
        })
    }

    fn internal_size(&mut self, ty: &Type) -> usize {
        self.internal_type(ty).bytes()
    }

    fn emit_allocate_internal(&mut self, size: usize) -> Operand {
        let result = self.temp(CoreType::I32);
        self.code.push_str(&format!("    i32.const {size}\n"));
        self.code.push_str("    call $allocate\n");
        self.emit_set(&result, CoreType::I32);
        result
    }

    fn emit_canonical_allocate(&mut self, size: &Operand, align: usize) -> Operand {
        let result = self.temp(CoreType::I32);
        self.emit_get(size, CoreType::I32);
        if align > 1 {
            self.code
                .push_str(&format!("    i32.const {}\n", align - 1));
            self.code.push_str("    i32.add\n");
        }
        self.code.push_str("    call $allocate\n");
        if align > 1 {
            self.code
                .push_str(&format!("    i32.const {}\n", align - 1));
            self.code.push_str("    i32.add\n");
            self.code
                .push_str(&format!("    i32.const {}\n", !(align as i32 - 1)));
            self.code.push_str("    i32.and\n");
        }
        self.emit_set(&result, CoreType::I32);
        result
    }

    fn emit_checked_byte_size(
        &mut self,
        length: &Operand,
        stride: usize,
        overhead: usize,
    ) -> Operand {
        if stride == 0 {
            return self.constant(CoreType::I32, &format!("    i32.const {overhead}\n"));
        }
        let max = (u32::MAX as usize - overhead) / stride;
        self.emit_get(length, CoreType::I32);
        self.code.push_str(&format!("    i32.const {max}\n"));
        self.code.push_str("    i32.gt_u\n");
        self.code.push_str("    (if (then unreachable))\n");
        let result = self.temp(CoreType::I32);
        self.emit_get(length, CoreType::I32);
        self.code.push_str(&format!("    i32.const {stride}\n"));
        self.code.push_str("    i32.mul\n");
        if overhead != 0 {
            self.code.push_str(&format!("    i32.const {overhead}\n"));
            self.code.push_str("    i32.add\n");
        }
        self.emit_set(&result, CoreType::I32);
        result
    }

    fn emit_dynamic_address(
        &mut self,
        base: &Operand,
        fixed_offset: usize,
        index: &Operand,
        stride: usize,
    ) -> Operand {
        let address = self.temp(CoreType::I32);
        self.emit_get(base, CoreType::I32);
        if fixed_offset != 0 {
            self.code
                .push_str(&format!("    i32.const {fixed_offset}\n    i32.add\n"));
        }
        if stride != 0 {
            self.emit_get(index, CoreType::I32);
            self.code.push_str(&format!(
                "    i32.const {stride}\n    i32.mul\n    i32.add\n"
            ));
        }
        self.emit_set(&address, CoreType::I32);
        address
    }

    fn emit_load_zero(&mut self, address: &Operand, ty: CoreType) -> Operand {
        let result = self.temp(ty);
        self.emit_get(address, CoreType::I32);
        self.code.push_str(&format!("    {}.load\n", ty.wat()));
        self.emit_set(&result, ty);
        result
    }

    fn emit_store_zero(&mut self, value: &Operand, address: &Operand, ty: CoreType) {
        self.emit_get(address, CoreType::I32);
        self.emit_get(value, ty);
        self.code.push_str(&format!("    {}.store\n", ty.wat()));
    }

    fn emit_length_constraint(&mut self, length: &Operand, expected: Option<usize>) {
        let Some(expected) = expected else {
            return;
        };
        self.emit_get(length, CoreType::I32);
        self.code
            .push_str(&format!("    i32.const {expected}\n    i32.ne\n"));
        self.code.push_str("    (if (then unreachable))\n");
    }

    fn assign_block_special(
        &mut self,
        block: &FinishedBlock,
        kind: SpecialKind,
        value: &Operand,
        ty: CoreType,
    ) {
        for id in &block.specials {
            if self.special_kinds.get(id) == Some(&kind) {
                let target = Operand {
                    source: OperandSource::Special(*id),
                    ty: Some(ty),
                };
                self.emit_get(value, ty);
                self.emit_set(&target, ty);
            }
        }
    }

    fn emit_string_lift(&mut self, operands: &[Operand], results: &mut Vec<Operand>) {
        let canonical_ptr = &operands[0];
        let length = &operands[1];
        let allocation_size = self.emit_checked_byte_size(length, 1, 4);
        let internal = self.emit_canonical_allocate(&allocation_size, 4);
        self.emit_store_zero(length, &internal, CoreType::I32);
        self.emit_get(&internal, CoreType::I32);
        self.code.push_str("    i32.const 4\n    i32.add\n");
        self.emit_get(canonical_ptr, CoreType::I32);
        self.emit_get(length, CoreType::I32);
        self.code.push_str("    memory.copy\n");
        results.push(internal);
    }

    fn emit_string_lower(&mut self, operands: &[Operand], results: &mut Vec<Operand>) {
        let internal = &operands[0];
        let length = self.emit_load_zero(internal, CoreType::I32);
        let canonical_ptr = self.emit_canonical_allocate(&length, 1);
        self.emit_get(&canonical_ptr, CoreType::I32);
        self.emit_get(internal, CoreType::I32);
        self.code.push_str("    i32.const 4\n    i32.add\n");
        self.emit_get(&length, CoreType::I32);
        self.code.push_str("    memory.copy\n");
        results.push(canonical_ptr);
        results.push(length);
    }

    fn emit_list_lift(&mut self, element: &Type, operands: &[Operand], results: &mut Vec<Operand>) {
        let mut blocks = self.pop_blocks(1);
        let Some(block) = blocks.pop() else {
            results.push(self.zero(CoreType::I32));
            return;
        };
        let canonical_ptr = &operands[0];
        let length = &operands[1];
        let constraint = self.lift_array_lengths.pop_front().unwrap_or(None);
        self.emit_length_constraint(length, constraint);
        let element_ty = self.internal_type(element);
        let stride = element_ty.bytes();
        let allocation_size = self.emit_checked_byte_size(length, stride, 8);
        let internal = self.emit_canonical_allocate(&allocation_size, 4);
        self.emit_store_zero(length, &internal, CoreType::I32);
        let zero = self.zero(CoreType::I32);
        let capacity_addr = self.emit_dynamic_address(&internal, 4, &zero, 0);
        self.emit_store_zero(length, &capacity_addr, CoreType::I32);

        let index = self.zero(CoreType::I32);
        let control = self.next_local;
        self.next_local += 1;
        self.code.push_str(&format!(
            "    (block $__restrict_external_list_lift_done_{control}\n"
        ));
        self.code.push_str(&format!(
            "      (loop $__restrict_external_list_lift_loop_{control}\n"
        ));
        self.emit_get(&index, CoreType::I32);
        self.emit_get(length, CoreType::I32);
        self.code.push_str("        i32.ge_u\n");
        self.code.push_str(&format!(
            "        br_if $__restrict_external_list_lift_done_{control}\n"
        ));
        let canonical_stride = self.sizes.size(element).size_wasm32();
        let element_base = self.emit_dynamic_address(canonical_ptr, 0, &index, canonical_stride);
        self.assign_block_special(&block, SpecialKind::IterBase, &element_base, CoreType::I32);
        self.code.push_str(&block.code);
        if block.results.len() != 1 {
            self.fail("Canonical list lift element block did not yield one Restrict value");
        } else {
            let internal_address = self.emit_dynamic_address(&internal, 8, &index, stride);
            self.emit_store_zero(&block.results[0], &internal_address, element_ty);
        }
        self.emit_get(&index, CoreType::I32);
        self.code.push_str("        i32.const 1\n        i32.add\n");
        self.emit_set(&index, CoreType::I32);
        self.code.push_str(&format!(
            "        br $__restrict_external_list_lift_loop_{control}\n      )\n    )\n"
        ));
        results.push(internal);
    }

    fn emit_list_lower(
        &mut self,
        element: &Type,
        operands: &[Operand],
        results: &mut Vec<Operand>,
    ) {
        let mut blocks = self.pop_blocks(1);
        let Some(block) = blocks.pop() else {
            results.push(self.zero(CoreType::I32));
            results.push(self.zero(CoreType::I32));
            return;
        };
        let internal = &operands[0];
        let length = self.emit_load_zero(internal, CoreType::I32);
        let constraint = self.lower_array_lengths.pop_front().unwrap_or(None);
        self.emit_length_constraint(&length, constraint);
        let canonical_stride = self.sizes.size(element).size_wasm32();
        let canonical_size = self.emit_checked_byte_size(&length, canonical_stride, 0);
        let canonical_ptr =
            self.emit_canonical_allocate(&canonical_size, self.sizes.align(element).align_wasm32());
        let element_ty = self.internal_type(element);
        let internal_stride = element_ty.bytes();
        let index = self.zero(CoreType::I32);
        let control = self.next_local;
        self.next_local += 1;
        self.code.push_str(&format!(
            "    (block $__restrict_external_list_lower_done_{control}\n"
        ));
        self.code.push_str(&format!(
            "      (loop $__restrict_external_list_lower_loop_{control}\n"
        ));
        self.emit_get(&index, CoreType::I32);
        self.emit_get(&length, CoreType::I32);
        self.code.push_str("        i32.ge_u\n");
        self.code.push_str(&format!(
            "        br_if $__restrict_external_list_lower_done_{control}\n"
        ));
        let internal_address = self.emit_dynamic_address(internal, 8, &index, internal_stride);
        let element_value = self.emit_load_zero(&internal_address, element_ty);
        let canonical_base = self.emit_dynamic_address(&canonical_ptr, 0, &index, canonical_stride);
        self.assign_block_special(&block, SpecialKind::IterElement, &element_value, element_ty);
        self.assign_block_special(
            &block,
            SpecialKind::IterBase,
            &canonical_base,
            CoreType::I32,
        );
        self.code.push_str(&block.code);
        if !block.results.is_empty() {
            self.fail("Canonical list lower element block unexpectedly yielded a value");
        }
        self.emit_get(&index, CoreType::I32);
        self.code.push_str("        i32.const 1\n        i32.add\n");
        self.emit_set(&index, CoreType::I32);
        self.code.push_str(&format!(
            "        br $__restrict_external_list_lower_loop_{control}\n      )\n    )\n"
        ));
        results.push(canonical_ptr);
        results.push(length);
    }

    fn emit_record_lift(
        &mut self,
        record: &wit_bindgen_core::wit_parser::Record,
        operands: &[Operand],
        results: &mut Vec<Operand>,
    ) {
        let mut field_types = Vec::with_capacity(record.fields.len());
        let mut size = 0usize;
        for field in &record.fields {
            let ty = self.internal_type(&field.ty);
            size += ty.bytes();
            field_types.push(ty);
        }
        let internal = self.emit_allocate_internal(size);
        let mut offset = 0usize;
        for ((_, operand), ty) in record.fields.iter().zip(operands).zip(field_types) {
            let zero = self.zero(CoreType::I32);
            let address = self.emit_dynamic_address(&internal, offset, &zero, 0);
            self.emit_store_zero(operand, &address, ty);
            offset += ty.bytes();
        }
        results.push(internal);
    }

    fn emit_record_lower(
        &mut self,
        record: &wit_bindgen_core::wit_parser::Record,
        operands: &[Operand],
        results: &mut Vec<Operand>,
    ) {
        let internal = &operands[0];
        let mut offset = 0usize;
        for field in &record.fields {
            let ty = self.internal_type(&field.ty);
            let zero = self.zero(CoreType::I32);
            let address = self.emit_dynamic_address(internal, offset, &zero, 0);
            results.push(self.emit_load_zero(&address, ty));
            offset += ty.bytes();
        }
    }

    fn emit_enum_lift(&mut self, operands: &[Operand], results: &mut Vec<Operand>) {
        let internal = self.emit_allocate_internal(4);
        self.emit_store_zero(&operands[0], &internal, CoreType::I32);
        results.push(internal);
    }

    fn emit_enum_lower(&mut self, operands: &[Operand], results: &mut Vec<Operand>) {
        results.push(self.emit_load_zero(&operands[0], CoreType::I32));
    }

    fn emit_variant_lift<'b, I>(
        &mut self,
        cases: I,
        operands: &[Operand],
        results: &mut Vec<Operand>,
        invert_result: bool,
    ) where
        I: IntoIterator<Item = Option<&'b Type>>,
    {
        let cases = cases.into_iter().collect::<Vec<_>>();
        let blocks = self.pop_blocks(cases.len());
        if blocks.len() != cases.len() {
            results.push(self.zero(CoreType::I32));
            return;
        }
        let mut payload_size = 0usize;
        for case in cases.iter().flatten() {
            payload_size = payload_size.max(self.internal_size(case));
        }
        let internal = self.emit_allocate_internal(4 + payload_size);
        let matched = self.zero(CoreType::I32);
        for (index, (case, block)) in cases.iter().zip(&blocks).enumerate() {
            self.emit_get(&operands[0], CoreType::I32);
            self.code.push_str(&format!(
                "    i32.const {index}\n    i32.eq\n    (if\n      (then\n"
            ));
            let internal_tag = if invert_result {
                cases.len() - 1 - index
            } else {
                index
            };
            let tag = self.constant(
                CoreType::I32,
                &format!("        i32.const {internal_tag}\n"),
            );
            self.emit_store_zero(&tag, &internal, CoreType::I32);
            self.code.push_str(&block.code);
            if let Some(case_ty) = case {
                if block.results.len() != 1 {
                    self.fail("Canonical variant lift payload block did not yield one value");
                } else {
                    let payload_ty = self.internal_type(case_ty);
                    let zero = self.zero(CoreType::I32);
                    let address = self.emit_dynamic_address(&internal, 4, &zero, 0);
                    self.emit_store_zero(&block.results[0], &address, payload_ty);
                }
            }
            let one = self.constant(CoreType::I32, "        i32.const 1\n");
            self.emit_get(&one, CoreType::I32);
            self.emit_set(&matched, CoreType::I32);
            self.code.push_str("      )\n    )\n");
        }
        self.emit_get(&matched, CoreType::I32);
        self.code
            .push_str("    i32.eqz\n    (if (then unreachable))\n");
        results.push(internal);
    }

    fn emit_variant_lower<'b, I>(
        &mut self,
        cases: I,
        result_types: &[WasmType],
        operands: &[Operand],
        results: &mut Vec<Operand>,
        invert_result: bool,
    ) where
        I: IntoIterator<Item = Option<&'b Type>>,
    {
        let cases = cases.into_iter().collect::<Vec<_>>();
        let blocks = self.pop_blocks(cases.len());
        if blocks.len() != cases.len() {
            for ty in result_types {
                results.push(self.zero(CoreType::from(*ty)));
            }
            return;
        }
        let internal = &operands[0];
        let tag = self.emit_load_zero(internal, CoreType::I32);
        let mut output_values = Vec::new();
        for ty in result_types {
            output_values.push(self.zero(CoreType::from(*ty)));
        }
        let matched = self.zero(CoreType::I32);
        for (index, (case, block)) in cases.iter().zip(&blocks).enumerate() {
            let internal_tag = if invert_result {
                cases.len() - 1 - index
            } else {
                index
            };
            self.emit_get(&tag, CoreType::I32);
            self.code.push_str(&format!(
                "    i32.const {internal_tag}\n    i32.eq\n    (if\n      (then\n"
            ));
            if let Some(case_ty) = case {
                let payload_ty = self.internal_type(case_ty);
                let zero = self.zero(CoreType::I32);
                let address = self.emit_dynamic_address(internal, 4, &zero, 0);
                let payload = self.emit_load_zero(&address, payload_ty);
                self.assign_block_special(block, SpecialKind::VariantPayload, &payload, payload_ty);
            }
            self.code.push_str(&block.code);
            if block.results.len() != output_values.len() {
                self.fail("Canonical variant lower block result arity mismatch");
            } else {
                for ((value, output), wasm_ty) in
                    block.results.iter().zip(&output_values).zip(result_types)
                {
                    let ty = CoreType::from(*wasm_ty);
                    self.emit_get(value, ty);
                    self.emit_set(output, ty);
                }
            }
            let one = self.constant(CoreType::I32, "        i32.const 1\n");
            self.emit_get(&one, CoreType::I32);
            self.emit_set(&matched, CoreType::I32);
            self.code.push_str("      )\n    )\n");
        }
        self.emit_get(&matched, CoreType::I32);
        self.code
            .push_str("    i32.eqz\n    (if (then unreachable))\n");
        results.extend(output_values);
    }

    fn emit_call_wasm(&mut self, operands: &[Operand], results: &mut Vec<Operand>) {
        let params = self.context.signature.params.clone();
        let result_types = self.context.signature.results.clone();
        if operands.len() != params.len() {
            self.fail(format!(
                "raw Canonical import expected {} operands, found {}",
                params.len(),
                operands.len()
            ));
        }
        for (operand, ty) in operands.iter().zip(&params) {
            self.emit_get(operand, CoreType::from(*ty));
        }
        self.code
            .push_str(&format!("    call ${}\n", self.context.raw_symbol));
        let mut reversed = Vec::with_capacity(result_types.len());
        for ty in result_types.iter().rev() {
            let result = self.temp(CoreType::from(*ty));
            self.emit_set(&result, CoreType::from(*ty));
            reversed.push(result);
        }
        reversed.reverse();
        results.extend(reversed);
    }

    fn finish(mut self) -> Result<String, ExternalImportAdapterError> {
        if !self.lift_array_lengths.is_empty() || !self.lower_array_lengths.is_empty() {
            self.fail(format!(
                "Canonical ABI traversal did not consume all list/array constraints ({} lift, {} lower)",
                self.lift_array_lengths.len(),
                self.lower_array_lengths.len()
            ));
        }
        if let Some(reason) = self.error.take() {
            return Err(ExternalImportAdapterError::UnsupportedCanonicalAdapter(
                self.context.binding.canonical_identity.clone(),
                reason,
            ));
        }
        if !self.code_stack.is_empty() || !self.blocks.is_empty() {
            return Err(ExternalImportAdapterError::UnsupportedCanonicalAdapter(
                self.context.binding.canonical_identity.clone(),
                "Canonical ABI instruction generator left unfinished blocks".to_string(),
            ));
        }
        let expected_results = usize::from(self.context.binding.result.is_some());
        if self.returned.len() != expected_results {
            return Err(ExternalImportAdapterError::UnsupportedCanonicalAdapter(
                self.context.binding.canonical_identity.clone(),
                format!(
                    "adapter returned {} Restrict values, expected {expected_results}",
                    self.returned.len()
                ),
            ));
        }
        if self.context.function.params.len() != self.context.binding.params.len() {
            return Err(ExternalImportAdapterError::UnsupportedCanonicalAdapter(
                self.context.binding.canonical_identity.clone(),
                "WIT and Restrict binding parameter counts diverged".to_string(),
            ));
        }

        for (id, ty) in &self.special_types {
            self.locals
                .insert(format!("__restrict_external_cabi_special_{id}"), *ty);
        }

        let mut output = String::new();
        output.push_str(&format!("  (func ${}", self.context.binding.local_function));
        for (index, param) in self.context.binding.params.iter().enumerate() {
            output.push_str(&format!(
                " (param $internal_param_{index} {})",
                component_internal_core_type(&param.ty).wat()
            ));
        }
        if let Some(result) = &self.context.binding.result {
            output.push_str(&format!(
                " (result {})",
                component_internal_core_type(result).wat()
            ));
        }
        output.push('\n');
        for (name, ty) in &self.locals {
            output.push_str(&format!("    (local ${name} {})\n", ty.wat()));
        }
        output.push_str("    global.get $current_arena\n");
        output.push_str("    i32.eqz\n");
        output.push_str("    (if (then unreachable))\n");
        output.push_str(&self.code);
        for operand in &self.returned {
            let expected = self
                .context
                .binding
                .result
                .as_ref()
                .map(component_internal_core_type)
                .unwrap_or(CoreType::I32);
            let name = match &operand.source {
                OperandSource::Local(name) => name.clone(),
                OperandSource::Special(id) => format!("__restrict_external_cabi_special_{id}"),
            };
            if operand.ty.is_some_and(|actual| actual != expected) {
                return Err(ExternalImportAdapterError::UnsupportedCanonicalAdapter(
                    self.context.binding.canonical_identity.clone(),
                    "final Restrict result type mismatch".to_string(),
                ));
            }
            output.push_str(&format!("    local.get ${name}\n"));
        }
        output.push_str("  )\n");
        Ok(output)
    }
}

impl Bindgen for ImportWatAbiBindgen<'_> {
    type Operand = Operand;

    fn emit(
        &mut self,
        _resolve: &Resolve,
        instruction: &Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        match instruction {
            Instruction::GetArg { nth } => {
                let ty = self
                    .context
                    .binding
                    .params
                    .get(*nth)
                    .map(|param| component_internal_core_type(&param.ty))
                    .unwrap_or(CoreType::I32);
                results.push(Operand {
                    source: OperandSource::Local(format!("internal_param_{nth}")),
                    ty: Some(ty),
                });
            }
            Instruction::I32Const { val } => {
                results.push(self.constant(CoreType::I32, &format!("    i32.const {val}\n")));
            }
            Instruction::ConstZero { tys } => {
                for ty in *tys {
                    results.push(self.zero(CoreType::from(*ty)));
                }
            }
            Instruction::Bitcasts { casts } => {
                for (operand, cast) in operands.iter().zip(*casts) {
                    results.push(self.emit_bitcast(operand, cast));
                }
            }

            Instruction::I32FromChar
            | Instruction::I32FromU32
            | Instruction::I32FromS32
            | Instruction::I32FromU16
            | Instruction::I32FromS16
            | Instruction::I32FromU8
            | Instruction::I32FromS8
            | Instruction::S8FromI32
            | Instruction::U8FromI32
            | Instruction::S16FromI32
            | Instruction::U16FromI32
            | Instruction::S32FromI32
            | Instruction::U32FromI32
            | Instruction::CharFromI32
            | Instruction::BoolFromI32
            | Instruction::I32FromBool => {
                results.push(self.coerce_noop(operands[0].clone(), CoreType::I32));
            }
            Instruction::I64FromU64
            | Instruction::I64FromS64
            | Instruction::S64FromI64
            | Instruction::U64FromI64 => {
                results.push(self.coerce_noop(operands[0].clone(), CoreType::I64));
            }
            Instruction::CoreF32FromF32 | Instruction::F32FromCoreF32 => {
                results.push(self.coerce_noop(operands[0].clone(), CoreType::F32));
            }
            Instruction::CoreF64FromF64 | Instruction::F64FromCoreF64 => {
                results.push(self.coerce_noop(operands[0].clone(), CoreType::F64));
            }

            Instruction::I32Load { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::I32, "i32.load"));
            }
            Instruction::I32Load8U { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::I32, "i32.load8_u"));
            }
            Instruction::I32Load8S { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::I32, "i32.load8_s"));
            }
            Instruction::I32Load16U { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::I32, "i32.load16_u"));
            }
            Instruction::I32Load16S { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::I32, "i32.load16_s"));
            }
            Instruction::I64Load { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::I64, "i64.load"));
            }
            Instruction::F32Load { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::F32, "f32.load"));
            }
            Instruction::F64Load { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::F64, "f64.load"));
            }
            Instruction::PointerLoad { offset } | Instruction::LengthLoad { offset } => {
                results.push(self.emit_load(&operands[0], *offset, CoreType::I32, "i32.load"));
            }

            Instruction::I32Store { offset } => self.emit_store(
                &operands[0],
                &operands[1],
                *offset,
                CoreType::I32,
                "i32.store",
            ),
            Instruction::I32Store8 { offset } => self.emit_store(
                &operands[0],
                &operands[1],
                *offset,
                CoreType::I32,
                "i32.store8",
            ),
            Instruction::I32Store16 { offset } => self.emit_store(
                &operands[0],
                &operands[1],
                *offset,
                CoreType::I32,
                "i32.store16",
            ),
            Instruction::I64Store { offset } => self.emit_store(
                &operands[0],
                &operands[1],
                *offset,
                CoreType::I64,
                "i64.store",
            ),
            Instruction::F32Store { offset } => self.emit_store(
                &operands[0],
                &operands[1],
                *offset,
                CoreType::F32,
                "f32.store",
            ),
            Instruction::F64Store { offset } => self.emit_store(
                &operands[0],
                &operands[1],
                *offset,
                CoreType::F64,
                "f64.store",
            ),
            Instruction::PointerStore { offset } | Instruction::LengthStore { offset } => {
                self.emit_store(
                    &operands[0],
                    &operands[1],
                    *offset,
                    CoreType::I32,
                    "i32.store",
                );
            }

            Instruction::StringLift => self.emit_string_lift(operands, results),
            Instruction::StringLower { .. } => self.emit_string_lower(operands, results),
            Instruction::ListLift { element, .. } | Instruction::ListCanonLift { element, .. } => {
                self.emit_list_lift(element, operands, results);
            }
            Instruction::ListLower { element, .. }
            | Instruction::ListCanonLower { element, .. } => {
                self.emit_list_lower(element, operands, results);
            }
            Instruction::IterElem { element } => {
                let ty = self.internal_type(element);
                results.push(self.special(SpecialKind::IterElement, Some(ty)));
            }
            Instruction::IterBasePointer => {
                results.push(self.special(SpecialKind::IterBase, Some(CoreType::I32)));
            }

            Instruction::RecordLift { record, .. } => {
                self.emit_record_lift(record, operands, results);
            }
            Instruction::RecordLower { record, .. } => {
                self.emit_record_lower(record, operands, results);
            }
            Instruction::TupleLift { tuple, .. } => {
                if tuple.types.is_empty() {
                    results.push(self.zero(CoreType::I32));
                } else {
                    self.fail("non-empty WIT tuple has no Restrict v0.1 boundary mapping");
                    results.push(self.zero(CoreType::I32));
                }
            }
            Instruction::TupleLower { tuple, .. } => {
                if !tuple.types.is_empty() {
                    self.fail("non-empty WIT tuple has no Restrict v0.1 boundary mapping");
                    for ty in &tuple.types {
                        let internal = self.internal_type(ty);
                        results.push(self.zero(internal));
                    }
                }
            }

            Instruction::VariantPayloadName => {
                results.push(self.special(SpecialKind::VariantPayload, None));
            }
            Instruction::VariantLift { variant, .. } => self.emit_variant_lift(
                variant.cases.iter().map(|case| case.ty.as_ref()),
                operands,
                results,
                false,
            ),
            Instruction::OptionLift { payload, .. } => {
                self.emit_variant_lift([None, Some(*payload)], operands, results, false);
            }
            Instruction::ResultLift { result, .. } => self.emit_variant_lift(
                [result.ok.as_ref(), result.err.as_ref()],
                operands,
                results,
                true,
            ),
            Instruction::VariantLower {
                variant,
                results: result_types,
                ..
            } => self.emit_variant_lower(
                variant.cases.iter().map(|case| case.ty.as_ref()),
                result_types,
                operands,
                results,
                false,
            ),
            Instruction::OptionLower {
                payload,
                results: result_types,
                ..
            } => self.emit_variant_lower(
                [None, Some(*payload)],
                result_types,
                operands,
                results,
                false,
            ),
            Instruction::ResultLower {
                result,
                results: result_types,
                ..
            } => self.emit_variant_lower(
                [result.ok.as_ref(), result.err.as_ref()],
                result_types,
                operands,
                results,
                true,
            ),
            Instruction::EnumLift { .. } => self.emit_enum_lift(operands, results),
            Instruction::EnumLower { .. } => self.emit_enum_lower(operands, results),

            Instruction::HandleLift { .. } | Instruction::HandleLower { .. } => {
                self.fail("resources/callables use a distinct external adapter path");
                results.push(self.coerce_noop(operands[0].clone(), CoreType::I32));
            }
            Instruction::CallWasm { .. } => self.emit_call_wasm(operands, results),
            Instruction::Return { amt, .. } => {
                if *amt != operands.len() {
                    self.fail("Canonical ABI return operand count mismatch");
                }
                self.returned = operands.clone();
            }
            Instruction::Malloc { size, align, .. } => {
                let size = self.constant(
                    CoreType::I32,
                    &format!("    i32.const {}\n", size.size_wasm32()),
                );
                results.push(self.emit_canonical_allocate(&size, align.align_wasm32()));
            }
            Instruction::GuestDeallocate { .. }
            | Instruction::GuestDeallocateString
            | Instruction::DropHandle { .. } => {}
            Instruction::GuestDeallocateList { .. } => {
                let _ = self.pop_blocks(1);
            }
            Instruction::GuestDeallocateVariant { blocks } => {
                let _ = self.pop_blocks(*blocks);
            }
            Instruction::GuestDeallocateMap { .. } => {
                let _ = self.pop_blocks(2);
            }
            Instruction::Flush { .. } => results.extend(operands.iter().cloned()),

            Instruction::CallInterface { .. }
            | Instruction::FlagsLift { .. }
            | Instruction::FlagsLower { .. }
            | Instruction::MapLift { .. }
            | Instruction::MapLower { .. }
            | Instruction::FixedLengthListLift { .. }
            | Instruction::FixedLengthListLower { .. }
            | Instruction::FixedLengthListLowerToMemory { .. }
            | Instruction::FixedLengthListLiftFromMemory { .. }
            | Instruction::IterMapKey { .. }
            | Instruction::IterMapValue { .. }
            | Instruction::FutureLift { .. }
            | Instruction::FutureLower { .. }
            | Instruction::StreamLift { .. }
            | Instruction::StreamLower { .. }
            | Instruction::ErrorContextLift
            | Instruction::ErrorContextLower
            | Instruction::AsyncTaskReturn { .. } => {
                self.fail(format!(
                    "unsupported Canonical ABI instruction: {instruction:?}"
                ));
                for _ in 0..instruction.results_len() {
                    results.push(self.zero(CoreType::I32));
                }
            }
        }
    }

    fn return_pointer(&mut self, size: ArchitectureSize, align: Alignment) -> Self::Operand {
        let size = self.constant(
            CoreType::I32,
            &format!("    i32.const {}\n", size.size_wasm32()),
        );
        self.emit_canonical_allocate(&size, align.align_wasm32())
    }

    fn push_block(&mut self) {
        self.code_stack.push(std::mem::take(&mut self.code));
        self.block_specials_stack.push(Vec::new());
    }

    fn finish_block(&mut self, operands: &mut Vec<Self::Operand>) {
        let code = std::mem::take(&mut self.code);
        let parent = self.code_stack.pop().unwrap_or_default();
        let specials = self.block_specials_stack.pop().unwrap_or_default();
        self.code = parent;
        self.blocks.push(FinishedBlock {
            code,
            results: operands.clone(),
            specials,
        });
    }

    fn sizes(&self) -> &SizeAlign {
        self.sizes
    }

    fn is_list_canonical(&self, _resolve: &Resolve, _element: &Type) -> bool {
        false
    }
}

fn collect_array_constraints<'a>(
    ty: &ComponentValueType,
    named_types: &BTreeMap<&'a str, &'a ComponentNamedTypeIr>,
    visiting: &mut BTreeSet<String>,
    output: &mut VecDeque<Option<usize>>,
) {
    match ty {
        ComponentValueType::List(element) => {
            collect_array_constraints(element, named_types, visiting, output);
            output.push_back(None);
        }
        ComponentValueType::Array { element, length } => {
            collect_array_constraints(element, named_types, visiting, output);
            output.push_back(Some(*length));
        }
        ComponentValueType::Option(inner) => {
            collect_array_constraints(inner, named_types, visiting, output);
        }
        ComponentValueType::Result { ok, err } => {
            collect_array_constraints(ok, named_types, visiting, output);
            collect_array_constraints(err, named_types, visiting, output);
        }
        ComponentValueType::Named(name) => {
            if !visiting.insert(name.clone()) {
                return;
            }
            if let Some(named) = named_types.get(name.as_str()) {
                match &named.kind {
                    ComponentNamedTypeKind::Record { fields } => {
                        for field in fields {
                            collect_array_constraints(&field.ty, named_types, visiting, output);
                        }
                    }
                    ComponentNamedTypeKind::Variant { cases } => {
                        for case in cases {
                            if let Some(payload) = &case.payload {
                                collect_array_constraints(payload, named_types, visiting, output);
                            }
                        }
                    }
                    ComponentNamedTypeKind::Enum { .. } => {}
                }
            }
            visiting.remove(name);
        }
        ComponentValueType::Unit
        | ComponentValueType::S32
        | ComponentValueType::S64
        | ComponentValueType::F64
        | ComponentValueType::Bool
        | ComponentValueType::Char
        | ComponentValueType::String
        | ComponentValueType::OwnResource(_)
        | ComponentValueType::OwnCallable(_) => {}
    }
}

fn component_internal_core_type(ty: &ComponentValueType) -> CoreType {
    match ty {
        ComponentValueType::S64 => CoreType::I64,
        ComponentValueType::F64 => CoreType::F64,
        ComponentValueType::Unit
        | ComponentValueType::S32
        | ComponentValueType::Bool
        | ComponentValueType::Char
        | ComponentValueType::String
        | ComponentValueType::List(_)
        | ComponentValueType::Array { .. }
        | ComponentValueType::Option(_)
        | ComponentValueType::Result { .. }
        | ComponentValueType::Named(_)
        | ComponentValueType::OwnResource(_)
        | ComponentValueType::OwnCallable(_) => CoreType::I32,
    }
}

fn component_value_is_pointer(ty: &ComponentValueType) -> bool {
    matches!(
        ty,
        ComponentValueType::String
            | ComponentValueType::List(_)
            | ComponentValueType::Array { .. }
            | ComponentValueType::Option(_)
            | ComponentValueType::Result { .. }
            | ComponentValueType::Named(_)
            | ComponentValueType::OwnResource(_)
            | ComponentValueType::OwnCallable(_)
    )
}

fn internal_core_type(resolve: &Resolve, ty: &Type) -> Result<CoreType, String> {
    match ty {
        Type::Bool
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::S8
        | Type::S16
        | Type::S32
        | Type::Char
        | Type::String
        | Type::ErrorContext => Ok(CoreType::I32),
        Type::U64 | Type::S64 => Ok(CoreType::I64),
        Type::F32 => Ok(CoreType::F32),
        Type::F64 => Ok(CoreType::F64),
        Type::Id(id) => match &resolve.types[*id].kind {
            TypeDefKind::Type(inner) => internal_core_type(resolve, inner),
            TypeDefKind::Record(_)
            | TypeDefKind::Resource
            | TypeDefKind::Handle(_)
            | TypeDefKind::Tuple(_)
            | TypeDefKind::Variant(_)
            | TypeDefKind::Enum(_)
            | TypeDefKind::Option(_)
            | TypeDefKind::Result(_)
            | TypeDefKind::List(_)
            | TypeDefKind::FixedLengthList(_, _) => Ok(CoreType::I32),
            other => Err(format!(
                "WIT {} has no Restrict v0.1 internal representation",
                other.as_str()
            )),
        },
    }
}

fn wat_type(ty: WasmType) -> &'static str {
    match ty {
        WasmType::I32 | WasmType::Pointer | WasmType::Length => "i32",
        WasmType::I64 | WasmType::PointerOrI64 => "i64",
        WasmType::F32 => "f32",
        WasmType::F64 => "f64",
    }
}

fn append_wat_string_contents(output: &mut String, value: &str) {
    for byte in value.bytes() {
        match byte {
            b'"' => output.push_str("\\\""),
            b'\\' => output.push_str("\\\\"),
            0x20..=0x7e => output.push(char::from(byte)),
            _ => output.push_str(&format!("\\{byte:02x}")),
        }
    }
}
