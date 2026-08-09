//! Restrict-owned synchronous WebAssembly Component compilation.
//!
//! This module provides checked Component Interface IR and deterministic WIT
//! for the v0.1 contract, compiler-owned Canonical ABI adapters, validated
//! Component packaging, explicit generic instances, and resource/callable
//! integration. The older scalar-only proof helper remains for regression
//! coverage, separate from the production `compile_component*` paths.

use crate::ast::{FunDecl, Program, TopDecl};
use crate::ir::builder::{
    CheckedFunctionIr, CheckedNamedTypeGraph, CheckedNamedTypeKind, CheckedNamedTypeResolution,
    CheckedProgramIr,
};
use crate::ir::FinalType;
use crate::type_checker::{ArrayLength, TypedType};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[cfg(not(target_arch = "wasm32"))]
use crate::codegen::{WasmCodeGen, WasmOptimizationLevel, WasmTargetProfile};
#[cfg(not(target_arch = "wasm32"))]
use crate::component_adapter::generate_component_adapter_core_wat;
#[cfg(not(target_arch = "wasm32"))]
use crate::component_generic::{
    plan_component_generic_exports, ComponentGenericPlan, ComponentNamedTypeResolutionError,
    ComponentNamedTypeResolver,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::component_resource_adapter::{
    generate_component_resource_adapter_core_wat, ComponentResourceAdapterLayout,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::component_resource_plan::{
    plan_component_resources, ComponentResourcePlan, ComponentResourceRootCatalog,
    PlannedComponentBoundaryType, PlannedComponentParam,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::component_selection_v2::{
    ComponentFunctionExportContract, ComponentSelectionContract, ComponentSourceVisibility,
    COMPONENT_SELECTION_SCHEMA_V2,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::ir::builder::build_checked_ir;
#[cfg(not(target_arch = "wasm32"))]
use crate::module::resolve_program_imports_for_file_with_package_roots;
#[cfg(not(target_arch = "wasm32"))]
use crate::parser::parse_program;
#[cfg(not(target_arch = "wasm32"))]
use crate::type_checker::TypeChecker;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

pub const COMPONENT_SELECTION_SCHEMA_VERSION: u32 = 1;
pub const COMPONENT_INTERFACE_IR_SCHEMA_VERSION: u32 = 1;
pub const COMPONENT_SEMANTIC_METADATA_SCHEMA_VERSION: u32 = 1;
pub const COMPONENT_ABI_PROFILE: &str = "canonical-v1-sync";
pub const COMPONENT_SCALAR_IMPLEMENTATION_SUBSET: &str =
    "restrict-component-interface-scalar-scaffold-v1";
pub const COMPONENT_RICH_INTERFACE_SUBSET: &str = "restrict-component-interface-rich-contract-v1";

/// Native compiler-library request used by artifact orchestrators.
///
/// The caller supplies an immutable source snapshot and package roots. Parsing,
/// module resolution, type checking, checked IR, interface construction,
/// Canonical ABI packaging, and validation all remain compiler-owned; Warder
/// must not reconstruct any of those facts from WAT or source text.
#[cfg(not(target_arch = "wasm32"))]
pub struct ComponentCompileRequest<'a> {
    pub source: &'a str,
    pub entry_path: &'a Path,
    pub package_roots: &'a [(String, PathBuf)],
    pub selection: &'a ComponentSelection,
    pub optimization_level: WasmOptimizationLevel,
    pub arena_size_bytes: u32,
}

/// Compiler-owned schema-v2 build request. Unlike schema v1, this contract can
/// select finite concrete instances of generic root functions and is also the
/// integration seam for synchronous resource/callable declarations.
#[cfg(not(target_arch = "wasm32"))]
pub struct ComponentCompileContractRequest<'a> {
    pub source: &'a str,
    pub entry_path: &'a Path,
    pub package_roots: &'a [(String, PathBuf)],
    pub selection: &'a ComponentSelectionContract,
    pub optimization_level: WasmOptimizationLevel,
    pub arena_size_bytes: u32,
}

/// Fully validated compiler output suitable for transactional publication.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentBuildArtifact {
    pub component_wasm: Vec<u8>,
    pub normalized_wit: String,
    /// Canonical semantic contract used for directional compatibility diffing.
    /// This deliberately excludes Core Wasm layout and optimizer facts.
    pub semantic_metadata: ComponentSemanticMetadata,
    pub interface_hash: [u8; 32],
    pub implementation_subset: String,
    pub package: String,
    pub world: String,
    pub abi_profile: String,
}

#[cfg(not(target_arch = "wasm32"))]
impl ComponentBuildArtifact {
    pub fn interface_hash_hex(&self) -> String {
        hex_lower(&self.interface_hash)
    }
}

/// Typed failures for the compiler-owned Component build pipeline.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Error)]
pub enum ComponentCompileError {
    #[error("Restrict source could not be parsed: {0}")]
    Parse(String),
    #[error("Restrict source contains unparsed input beginning with {0:?}")]
    TrailingInput(String),
    #[error("root Component export catalog is invalid: {0}")]
    RootCatalog(#[from] ComponentInterfaceError),
    #[error("Restrict module resolution failed: {0}")]
    ModuleResolution(String),
    #[error("Restrict type checking failed: {0}")]
    TypeCheck(String),
    #[error("checked Restrict IR construction failed: {0}")]
    CheckedIr(String),
    #[error("Component schema-v2 contract is invalid: {0}")]
    Contract(String),
    #[error("Component generic specialization failed: {0}")]
    Generic(String),
    #[error("Component resource/callable planning failed: {0}")]
    Resource(String),
    #[error("Restrict Core Wasm generation failed: {0}")]
    CoreCodegen(String),
    #[error("Restrict Canonical ABI adapter generation failed: {0}")]
    Adapter(String),
    #[error("Restrict Component packaging failed: {0}")]
    Packaging(#[from] ScalarComponentPackagingError),
}

/// Manifest-neutral selection of one Restrict-owned Component interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSelection {
    pub schema_version: u32,
    pub package: String,
    pub world: String,
    pub abi_profile: String,
    pub exports: Vec<ComponentExportSelection>,
}

impl ComponentSelection {
    pub fn v1(
        package: impl Into<String>,
        world: impl Into<String>,
        abi_profile: impl Into<String>,
        exports: Vec<ComponentExportSelection>,
    ) -> Self {
        Self {
            schema_version: COMPONENT_SELECTION_SCHEMA_VERSION,
            package: package.into(),
            world: world.into(),
            abi_profile: abi_profile.into(),
            exports,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentExportSelection {
    pub interface: String,
    pub source_function: String,
    pub external_name: String,
}

impl ComponentExportSelection {
    pub fn new(
        interface: impl Into<String>,
        source_function: impl Into<String>,
        external_name: impl Into<String>,
    ) -> Self {
        Self {
            interface: interface.into(),
            source_function: source_function.into(),
            external_name: external_name.into(),
        }
    }
}

/// Public functions declared by the root source before import resolution.
///
/// Keeping this catalog separate from the flattened resolved program prevents
/// a selection from accidentally publishing an imported `pub fun`.
#[derive(Debug, Clone, PartialEq)]
pub struct RootPublicFunctionCatalog {
    functions: BTreeMap<String, RootPublicFunction>,
}

#[derive(Debug, Clone, PartialEq)]
struct RootPublicFunction {
    declaration: FunDecl,
}

impl RootPublicFunctionCatalog {
    pub fn from_unresolved_root(program: &Program) -> Result<Self, ComponentInterfaceError> {
        let mut functions = BTreeMap::new();
        for declaration in &program.declarations {
            let TopDecl::Export(export) = declaration else {
                continue;
            };
            let TopDecl::Function(function) = export.item.as_ref() else {
                continue;
            };
            if functions
                .insert(
                    function.name.clone(),
                    RootPublicFunction {
                        declaration: function.clone(),
                    },
                )
                .is_some()
            {
                return Err(ComponentInterfaceError::DuplicateRootPublicFunction(
                    function.name.clone(),
                ));
            }
        }
        Ok(Self { functions })
    }

    pub fn contains(&self, function: &str) -> bool {
        self.functions.contains_key(function)
    }

    fn matches_resolved_root(&self, function: &str, resolved: &Program) -> bool {
        let Some(expected) = self.functions.get(function) else {
            return false;
        };
        resolved.declarations.iter().any(|declaration| {
            matches!(
                declaration,
                TopDecl::Export(export)
                    if matches!(export.item.as_ref(), TopDecl::Function(actual) if actual == &expected.declaration)
            )
        })
    }
}

/// Validate the deliberately conservative WIT identifier subset used by
/// component manifest schema v1.
pub fn is_component_schema_v1_wit_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    let mut previous_was_hyphen = false;
    for ch in chars {
        match ch {
            'a'..='z' | '0'..='9' => previous_was_hyphen = false,
            '-' if !previous_was_hyphen => previous_was_hyphen = true,
            _ => return false,
        }
    }
    !previous_was_hyphen
        && wit_parser::validate_id(value).is_ok()
        && !is_unescaped_wit_keyword(value)
}

fn is_unescaped_wit_keyword(value: &str) -> bool {
    matches!(
        value,
        "use"
            | "type"
            | "func"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "s8"
            | "s16"
            | "s32"
            | "s64"
            | "f32"
            | "f64"
            | "char"
            | "resource"
            | "own"
            | "borrow"
            | "record"
            | "flags"
            | "variant"
            | "enum"
            | "bool"
            | "string"
            | "option"
            | "result"
            | "future"
            | "stream"
            | "error-context"
            | "list"
            | "map"
            | "as"
            | "from"
            | "static"
            | "interface"
            | "tuple"
            | "world"
            | "import"
            | "export"
            | "package"
            | "constructor"
            | "include"
            | "with"
            | "async"
    )
}

/// WIT-facing logical value kinds in the synchronous Component ABI.
///
/// `Unit` is retained for adapter provenance. It is omitted from WIT
/// parameters and results because it carries no Component value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComponentValueType {
    Unit,
    S32,
    S64,
    F64,
    Bool,
    Char,
    String,
    List(Box<ComponentValueType>),
    /// WIT lowers fixed arrays as `list<T>` while the exact source length stays
    /// in stable metadata and is checked by both adapter directions.
    Array {
        element: Box<ComponentValueType>,
        length: usize,
    },
    Option(Box<ComponentValueType>),
    Result {
        ok: Box<ComponentValueType>,
        err: Box<ComponentValueType>,
    },
    /// Reference to a declaration in the surrounding Component interface.
    Named(String),
    /// Direct affine ownership of an explicitly classified WIT resource.
    OwnResource(String),
    /// Direct affine ownership of one signature-specific callable resource.
    OwnCallable(String),
}

impl ComponentValueType {
    fn scalar_wit_name(&self) -> Option<&'static str> {
        match self {
            Self::Unit => None,
            Self::S32 => Some("s32"),
            Self::S64 => Some("s64"),
            Self::F64 => Some("f64"),
            Self::Bool => Some("bool"),
            Self::Char => Some("char"),
            Self::String => Some("string"),
            Self::List(_)
            | Self::Array { .. }
            | Self::Option(_)
            | Self::Result { .. }
            | Self::Named(_)
            | Self::OwnResource(_)
            | Self::OwnCallable(_) => None,
        }
    }

    fn semantic_tag(&self) -> u8 {
        match self {
            Self::Unit => 0,
            Self::S32 => 1,
            Self::S64 => 2,
            Self::F64 => 3,
            Self::Bool => 4,
            Self::Char => 5,
            Self::String => 6,
            Self::List(_) => 7,
            Self::Array { .. } => 8,
            Self::Option(_) => 9,
            Self::Result { .. } => 10,
            Self::Named(_) => 11,
            Self::OwnResource(_) => 12,
            Self::OwnCallable(_) => 13,
        }
    }

    fn is_scalar_or_unit(&self) -> bool {
        matches!(
            self,
            Self::Unit | Self::S32 | Self::S64 | Self::F64 | Self::Bool | Self::Char
        )
    }

    fn write_wit(&self, output: &mut String) {
        if let Some(name) = self.scalar_wit_name() {
            output.push_str(name);
            return;
        }
        match self {
            Self::Unit => output.push_str("tuple<>"),
            Self::List(element) | Self::Array { element, .. } => {
                output.push_str("list<");
                element.write_wit(output);
                output.push('>');
            }
            Self::Option(inner) => {
                output.push_str("option<");
                inner.write_wit(output);
                output.push('>');
            }
            Self::Result { ok, err } => match (ok.as_ref(), err.as_ref()) {
                (Self::Unit, Self::Unit) => output.push_str("result"),
                (Self::Unit, err) => {
                    output.push_str("result<_, ");
                    err.write_wit(output);
                    output.push('>');
                }
                (ok, Self::Unit) => {
                    output.push_str("result<");
                    ok.write_wit(output);
                    output.push('>');
                }
                (ok, err) => {
                    output.push_str("result<");
                    ok.write_wit(output);
                    output.push_str(", ");
                    err.write_wit(output);
                    output.push('>');
                }
            },
            Self::Named(name) => write_wit_identifier(output, name),
            Self::OwnResource(name) | Self::OwnCallable(name) => {
                output.push_str("own<");
                write_wit_identifier(output, name);
                output.push('>');
            }
            Self::S32 | Self::S64 | Self::F64 | Self::Bool | Self::Char | Self::String => {
                unreachable!("scalar WIT names returned above")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentParamIr {
    pub source_index: usize,
    pub source_name: String,
    /// Normalized WIT identifier without the optional `%` keyword escape.
    pub name: String,
    pub ty: ComponentValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentFunctionIr {
    /// Compiler-internal source symbol used by later adapter lowering.
    pub source_function: String,
    /// Stable WIT-visible function name selected by the package owner.
    pub name: String,
    pub params: Vec<ComponentParamIr>,
    pub result: ComponentValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInterfaceDeclIr {
    pub name: String,
    pub types: Vec<ComponentNamedTypeIr>,
    pub resources: Vec<ComponentResourceIr>,
    pub callables: Vec<ComponentCallableIr>,
    pub functions: Vec<ComponentFunctionIr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentResourceReceiverIr {
    Borrow,
    Own,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentResourceIr {
    pub name: String,
    pub source_record: String,
    pub destructor_source_function: String,
    pub methods: Vec<ComponentResourceMethodIr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentResourceMethodIr {
    pub source_function: String,
    pub name: String,
    pub receiver: ComponentResourceReceiverIr,
    pub receiver_name: String,
    pub params: Vec<ComponentParamIr>,
    pub result: ComponentValueType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentCallableMultiplicityIr {
    Once,
    /// Reserved compatibility identity for the v0.2 callable contract.
    /// The v0.1 schema-v2 compiler rejects this before Component IR lowering.
    Many,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentCallableIr {
    pub name: String,
    pub params: Vec<ComponentParamIr>,
    pub result: ComponentValueType,
    pub multiplicity: ComponentCallableMultiplicityIr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentNamedTypeIr {
    pub name: String,
    pub source_type: String,
    pub kind: ComponentNamedTypeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentNamedTypeKind {
    Record { fields: Vec<ComponentRecordFieldIr> },
    Enum { cases: Vec<ComponentEnumCaseIr> },
    Variant { cases: Vec<ComponentVariantCaseIr> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRecordFieldIr {
    pub source_index: usize,
    pub source_name: String,
    pub name: String,
    pub ty: ComponentValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEnumCaseIr {
    pub source_index: usize,
    pub source_name: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentVariantCaseIr {
    pub source_index: usize,
    pub source_name: String,
    pub name: String,
    pub payload: Option<ComponentValueType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentWorldIr {
    pub name: String,
    pub exported_interfaces: Vec<String>,
}

/// Component contract IR, intentionally independent from Core Wasm layouts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInterfaceIr {
    pub schema_version: u32,
    pub implementation_subset: String,
    pub abi_profile: String,
    pub package: String,
    pub world: ComponentWorldIr,
    pub interfaces: Vec<ComponentInterfaceDeclIr>,
}

/// Versioned semantic contract used for interface compatibility hashing.
///
/// It excludes internal source symbols, WIT formatting, Core Wasm bytes, and
/// compiler-owned layout identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSemanticMetadata {
    pub schema_version: u32,
    pub implementation_subset: String,
    pub abi_profile: String,
    pub package: String,
    pub world: String,
    pub interfaces: Vec<ComponentInterfaceMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInterfaceMetadata {
    pub name: String,
    pub types: Vec<ComponentNamedTypeMetadata>,
    pub resources: Vec<ComponentResourceMetadata>,
    pub callables: Vec<ComponentCallableMetadata>,
    pub functions: Vec<ComponentFunctionMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentResourceMetadata {
    pub name: String,
    pub methods: Vec<ComponentResourceMethodMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentResourceMethodMetadata {
    pub name: String,
    pub receiver: ComponentResourceReceiverIr,
    pub params: Vec<ComponentParamMetadata>,
    pub result: ComponentValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentCallableMetadata {
    pub name: String,
    pub params: Vec<ComponentParamMetadata>,
    pub result: ComponentValueType,
    pub multiplicity: ComponentCallableMultiplicityIr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentNamedTypeMetadata {
    pub name: String,
    pub kind: ComponentNamedTypeMetadataKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentNamedTypeMetadataKind {
    Record {
        fields: Vec<ComponentRecordFieldMetadata>,
    },
    Enum {
        cases: Vec<String>,
    },
    Variant {
        cases: Vec<ComponentVariantCaseMetadata>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRecordFieldMetadata {
    pub name: String,
    pub ty: ComponentValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentVariantCaseMetadata {
    pub name: String,
    pub payload: Option<ComponentValueType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentFunctionMetadata {
    pub name: String,
    pub params: Vec<ComponentParamMetadata>,
    pub result: ComponentValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentParamMetadata {
    pub name: String,
    pub ty: ComponentValueType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInterfaceArtifact {
    pub ir: ComponentInterfaceIr,
    pub normalized_wit: String,
    pub metadata: ComponentSemanticMetadata,
    pub interface_hash: [u8; 32],
}

/// Native-only proof artifact for the initial scalar Component path.
///
/// This remains a compiler-internal regression scaffold. The production B1
/// paths use `ComponentBuildArtifact` and the compiler-owned rich/resource
/// adapters instead of publishing this historical scalar proof directly.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentalScalarComponentArtifact {
    /// Validated Core Wasm with Standard32 canonical export aliases inserted.
    pub aliased_core_wasm: Vec<u8>,
    /// Validated Core Wasm carrying the WIT `component-type` metadata section.
    pub embedded_core_wasm: Vec<u8>,
    /// Validated WebAssembly Component that targets the generated WIT world.
    pub component_wasm: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
struct ValidatedCoreModule {
    wasm: Vec<u8>,
    exports: BTreeMap<String, (wasmparser::ExternalKind, u32)>,
}

impl ComponentInterfaceArtifact {
    pub fn interface_hash_hex(&self) -> String {
        hex_lower(&self.interface_hash)
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ComponentInterfaceError {
    #[error("unsupported Component selection schema version {0}; expected version 1")]
    UnsupportedSelectionVersion(u32),
    #[error("unsupported Component ABI profile '{0}'; expected 'canonical-v1-sync'")]
    UnsupportedAbiProfile(String),
    #[error("Component selection must contain at least one explicit export")]
    EmptySelection,
    #[error("root source declares public function '{0}' more than once")]
    DuplicateRootPublicFunction(String),
    #[error("Component selection names source function '{0}' more than once")]
    DuplicateSourceSelection(String),
    #[error("Component selection publishes WIT function name '{0}' more than once")]
    DuplicateExternalName(String),
    #[error("Component selection function '{0}' is not a root-source public function")]
    NotRootPublicFunction(String),
    #[error("checked IR is inconsistent with its construction-time facts")]
    InvalidCheckedIr,
    #[error("resolved source program does not match the checked compiler state")]
    CheckedSourceProgramMismatch,
    #[error("checked IR has no function '{0}' selected by the root package")]
    MissingCheckedFunction(String),
    #[error("checked function '{0}' is not marked as a source export")]
    CheckedFunctionNotPublic(String),
    #[error("selected root function '{0}' does not match the resolved source declaration")]
    ResolvedRootDeclarationMismatch(String),
    #[error("async function '{0}' is outside the synchronous Component interface contract")]
    AsyncFunctionUnsupported(String),
    #[error("generic function '{0}' requires an explicit concrete instantiation before Component interface lowering")]
    OpenGenericUnsupported(String),
    #[error("temporal function '{0}' is outside the synchronous Component interface contract")]
    TemporalFunctionUnsupported(String),
    #[error("function '{function}' parameter '{parameter}' cannot be normalized as a WIT identifier: {reason}")]
    InvalidParameterName {
        function: String,
        parameter: String,
        reason: String,
    },
    #[error("function '{function}' has multiple parameters that normalize to WIT name '{name}'")]
    ParameterNameCollision { function: String, name: String },
    #[error("function '{function}' {position} type '{ty}' is outside the synchronous Component interface contract")]
    UnsupportedBoundaryType {
        function: String,
        position: String,
        ty: String,
    },
    #[error(
        "function '{function}' {position} uses an open generic Component boundary type '{ty}'"
    )]
    OpenGenericBoundaryType {
        function: String,
        position: String,
        ty: String,
    },
    #[error("function '{function}' {position} uses Array with an unresolved exact length")]
    UnknownArrayLength { function: String, position: String },
    #[error("checked named type metadata is missing for Component boundary type '{0}'")]
    MissingNamedType(String),
    #[error("Component type name '{name}' collides between '{first}' and '{second}'")]
    NamedTypeCollision {
        name: String,
        first: String,
        second: String,
    },
    #[error("Component named type '{ty}' has fields or cases that normalize to duplicate WIT name '{name}'")]
    NamedMemberCollision { ty: String, name: String },
    #[error("Component named type '{0}' contains unsupported or unresolved checked metadata")]
    InvalidNamedType(String),
    #[error("generated WIT failed in-process validation: {0}")]
    GeneratedWitRejected(String),
}

/// Build deterministic synchronous Component Interface IR and validated WIT.
///
/// This function does not produce a WebAssembly component. Both the raw-root
/// catalog and the explicit selection are required so `pub` alone cannot
/// publish an ABI and imported public functions cannot be selected by accident.
pub fn build_component_interface(
    root_public: &RootPublicFunctionCatalog,
    resolved_program: &Program,
    checked_program: &CheckedProgramIr,
    selection: &ComponentSelection,
) -> Result<ComponentInterfaceArtifact, ComponentInterfaceError> {
    if selection.schema_version != COMPONENT_SELECTION_SCHEMA_VERSION {
        return Err(ComponentInterfaceError::UnsupportedSelectionVersion(
            selection.schema_version,
        ));
    }
    if selection.abi_profile != COMPONENT_ABI_PROFILE {
        return Err(ComponentInterfaceError::UnsupportedAbiProfile(
            selection.abi_profile.clone(),
        ));
    }
    if selection.exports.is_empty() {
        return Err(ComponentInterfaceError::EmptySelection);
    }
    if !checked_program.is_unmodified() || checked_program.validate_lowering_summaries().is_err() {
        return Err(ComponentInterfaceError::InvalidCheckedIr);
    }
    if !checked_program.matches_source_program(resolved_program) {
        return Err(ComponentInterfaceError::CheckedSourceProgramMismatch);
    }

    let checked_by_name = checked_program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut selected_sources = BTreeSet::new();
    let mut interfaces = BTreeMap::<String, PendingComponentInterface>::new();

    for export in &selection.exports {
        if !selected_sources.insert(export.source_function.clone()) {
            return Err(ComponentInterfaceError::DuplicateSourceSelection(
                export.source_function.clone(),
            ));
        }
        let Some(root_function) = root_public.functions.get(&export.source_function) else {
            return Err(ComponentInterfaceError::NotRootPublicFunction(
                export.source_function.clone(),
            ));
        };
        if !root_public.matches_resolved_root(&export.source_function, resolved_program) {
            return Err(ComponentInterfaceError::ResolvedRootDeclarationMismatch(
                export.source_function.clone(),
            ));
        }
        if root_function.declaration.is_async {
            return Err(ComponentInterfaceError::AsyncFunctionUnsupported(
                export.source_function.clone(),
            ));
        }
        let checked = checked_by_name
            .get(export.source_function.as_str())
            .ok_or_else(|| {
                ComponentInterfaceError::MissingCheckedFunction(export.source_function.clone())
            })?;
        if !checked.lowering.source_exported {
            return Err(ComponentInterfaceError::CheckedFunctionNotPublic(
                export.source_function.clone(),
            ));
        }

        let (function, named_types) =
            lower_checked_function(checked, export, &checked_program.named_types)?;
        let interface = interfaces.entry(export.interface.clone()).or_default();
        for named_type in named_types {
            if let Some(existing) = interface.types.get(&named_type.name) {
                if existing != &named_type {
                    return Err(ComponentInterfaceError::NamedTypeCollision {
                        name: named_type.name,
                        first: existing.source_type.clone(),
                        second: named_type.source_type,
                    });
                }
            } else {
                interface.types.insert(named_type.name.clone(), named_type);
            }
        }
        if interface
            .functions
            .insert(export.external_name.clone(), function)
            .is_some()
        {
            return Err(ComponentInterfaceError::DuplicateExternalName(format!(
                "{}.{}",
                export.interface, export.external_name
            )));
        }
    }

    let interfaces = interfaces
        .into_iter()
        .map(|(name, interface)| ComponentInterfaceDeclIr {
            name,
            types: interface.types.into_values().collect(),
            resources: interface.resources.into_values().collect(),
            callables: interface.callables.into_values().collect(),
            functions: interface.functions.into_values().collect(),
        })
        .collect::<Vec<_>>();
    let exported_interfaces = interfaces
        .iter()
        .map(|interface| interface.name.clone())
        .collect();
    let ir = ComponentInterfaceIr {
        schema_version: COMPONENT_INTERFACE_IR_SCHEMA_VERSION,
        implementation_subset: component_implementation_subset(&interfaces).to_string(),
        abi_profile: selection.abi_profile.clone(),
        package: selection.package.clone(),
        world: ComponentWorldIr {
            name: selection.world.clone(),
            exported_interfaces,
        },
        interfaces,
    };
    let normalized_wit = emit_deterministic_wit(&ir);
    validate_generated_wit(&normalized_wit)?;
    let metadata = component_semantic_metadata(&ir);
    let interface_hash = component_interface_hash(&normalized_wit, &metadata);

    Ok(ComponentInterfaceArtifact {
        ir,
        normalized_wit,
        metadata,
        interface_hash,
    })
}

fn component_implementation_subset(interfaces: &[ComponentInterfaceDeclIr]) -> &'static str {
    if interfaces.iter().any(|interface| {
        !interface.types.is_empty()
            || !interface.resources.is_empty()
            || !interface.callables.is_empty()
            || interface.functions.iter().any(|function| {
                function
                    .params
                    .iter()
                    .any(|param| !param.ty.is_scalar_or_unit())
                    || !function.result.is_scalar_or_unit()
            })
    }) {
        COMPONENT_RICH_INTERFACE_SUBSET
    } else {
        COMPONENT_SCALAR_IMPLEMENTATION_SUBSET
    }
}

fn first_rich_boundary(ir: &ComponentInterfaceIr) -> Option<(&str, &str)> {
    ir.interfaces.iter().find_map(|interface| {
        interface.functions.iter().find_map(|function| {
            let is_rich = function
                .params
                .iter()
                .any(|param| !param.ty.is_scalar_or_unit())
                || !function.result.is_scalar_or_unit();
            is_rich.then_some((interface.name.as_str(), function.name.as_str()))
        })
    })
}

#[derive(Default)]
struct PendingComponentInterface {
    types: BTreeMap<String, ComponentNamedTypeIr>,
    resources: BTreeMap<String, ComponentResourceIr>,
    callables: BTreeMap<String, ComponentCallableIr>,
    functions: BTreeMap<String, ComponentFunctionIr>,
}

fn lower_checked_function(
    checked: &CheckedFunctionIr,
    selection: &ComponentExportSelection,
    named_type_graph: &CheckedNamedTypeGraph,
) -> Result<(ComponentFunctionIr, Vec<ComponentNamedTypeIr>), ComponentInterfaceError> {
    if !checked.lowering.declared_type_params.is_empty() || !checked.monomorphic {
        return Err(ComponentInterfaceError::OpenGenericUnsupported(
            checked.name.clone(),
        ));
    }
    if !checked.lowering.temporal_constraints.is_empty() {
        return Err(ComponentInterfaceError::TemporalFunctionUnsupported(
            checked.name.clone(),
        ));
    }

    let mut lowerer = ComponentTypeLowerer::new(&checked.name, named_type_graph);
    let mut params = Vec::with_capacity(checked.params.len());
    let mut normalized_names = BTreeSet::new();
    for (source_index, param) in checked.params.iter().enumerate() {
        let position = format!("parameter '{}'", param.name);
        let ty = lowerer.lower(&param.final_type, &position)?;
        let name = if ty == ComponentValueType::Unit {
            format!("unit-{source_index}")
        } else {
            normalize_component_identifier(&param.name).map_err(|reason| {
                ComponentInterfaceError::InvalidParameterName {
                    function: checked.name.clone(),
                    parameter: param.name.clone(),
                    reason,
                }
            })?
        };
        if ty != ComponentValueType::Unit && !normalized_names.insert(name.clone()) {
            return Err(ComponentInterfaceError::ParameterNameCollision {
                function: checked.name.clone(),
                name,
            });
        }
        params.push(ComponentParamIr {
            source_index,
            source_name: param.name.clone(),
            name,
            ty,
        });
    }
    let result = lowerer.lower(&checked.return_type, "return")?;

    Ok((
        ComponentFunctionIr {
            source_function: checked.name.clone(),
            name: selection.external_name.clone(),
            params,
            result,
        },
        lowerer.finish(),
    ))
}

struct ComponentTypeLowerer<'a> {
    function: &'a str,
    graph: &'a CheckedNamedTypeGraph,
    declarations: BTreeMap<String, ComponentNamedTypeIr>,
    source_by_name: BTreeMap<String, String>,
    visiting: BTreeSet<String>,
}

impl<'a> ComponentTypeLowerer<'a> {
    fn new(function: &'a str, graph: &'a CheckedNamedTypeGraph) -> Self {
        Self {
            function,
            graph,
            declarations: BTreeMap::new(),
            source_by_name: BTreeMap::new(),
            visiting: BTreeSet::new(),
        }
    }

    fn finish(self) -> Vec<ComponentNamedTypeIr> {
        self.declarations.into_values().collect()
    }

    fn lower(
        &mut self,
        final_type: &FinalType,
        position: &str,
    ) -> Result<ComponentValueType, ComponentInterfaceError> {
        let ty = final_type.as_typed_type();
        match ty {
            TypedType::Unit => Ok(ComponentValueType::Unit),
            TypedType::Int32 => Ok(ComponentValueType::S32),
            TypedType::Int64 => Ok(ComponentValueType::S64),
            TypedType::Float64 => Ok(ComponentValueType::F64),
            TypedType::Boolean => Ok(ComponentValueType::Bool),
            TypedType::Char => Ok(ComponentValueType::Char),
            TypedType::String => Ok(ComponentValueType::String),
            TypedType::List(inner) => Ok(ComponentValueType::List(Box::new(
                self.lower_typed(inner, position)?,
            ))),
            TypedType::Array(inner, ArrayLength::Known(length)) => Ok(ComponentValueType::Array {
                element: Box::new(self.lower_typed(inner, position)?),
                length: *length,
            }),
            TypedType::Array(_, ArrayLength::AnyInternal) => {
                Err(ComponentInterfaceError::UnknownArrayLength {
                    function: self.function.to_string(),
                    position: position.to_string(),
                })
            }
            TypedType::Option(inner) => Ok(ComponentValueType::Option(Box::new(
                self.lower_typed(inner, position)?,
            ))),
            TypedType::Result(ok, err) => Ok(ComponentValueType::Result {
                ok: Box::new(self.lower_typed(ok, position)?),
                err: Box::new(self.lower_typed(err, position)?),
            }),
            TypedType::Record { .. } | TypedType::Enum { .. } => {
                self.lower_named(final_type, position)
            }
            TypedType::TypeParam(_) => Err(ComponentInterfaceError::OpenGenericBoundaryType {
                function: self.function.to_string(),
                position: position.to_string(),
                ty: crate::type_checker::format_typed_type(ty),
            }),
            TypedType::Function { .. }
            | TypedType::InferVar(_)
            | TypedType::Projection { .. }
            | TypedType::Temporal { .. } => Err(ComponentInterfaceError::UnsupportedBoundaryType {
                function: self.function.to_string(),
                position: position.to_string(),
                ty: crate::type_checker::format_typed_type(ty),
            }),
        }
    }

    fn lower_typed(
        &mut self,
        ty: &TypedType,
        position: &str,
    ) -> Result<ComponentValueType, ComponentInterfaceError> {
        let final_type = FinalType::new(ty.clone()).map_err(|_| {
            ComponentInterfaceError::UnsupportedBoundaryType {
                function: self.function.to_string(),
                position: position.to_string(),
                ty: crate::type_checker::format_typed_type(ty),
            }
        })?;
        self.lower(&final_type, position)
    }

    fn lower_named(
        &mut self,
        final_type: &FinalType,
        position: &str,
    ) -> Result<ComponentValueType, ComponentInterfaceError> {
        let source_type = crate::type_checker::format_typed_type(final_type.as_typed_type());
        let (resolution, checked_kind) = {
            let checked = self
                .graph
                .find(final_type)
                .ok_or_else(|| ComponentInterfaceError::MissingNamedType(source_type.clone()))?;
            (checked.resolution.clone(), checked.kind.clone())
        };
        if !matches!(resolution, CheckedNamedTypeResolution::Concrete) {
            return Err(ComponentInterfaceError::OpenGenericBoundaryType {
                function: self.function.to_string(),
                position: position.to_string(),
                ty: source_type,
            });
        }
        let name = component_named_type_name(final_type)?;
        if let Some(first) = self.source_by_name.get(&name) {
            if first != &source_type {
                return Err(ComponentInterfaceError::NamedTypeCollision {
                    name,
                    first: first.clone(),
                    second: source_type,
                });
            }
        } else {
            self.source_by_name
                .insert(name.clone(), source_type.clone());
        }
        if self.declarations.contains_key(&name) {
            return Ok(ComponentValueType::Named(name));
        }
        if !self.visiting.insert(source_type.clone()) {
            return Err(ComponentInterfaceError::InvalidNamedType(source_type));
        }

        let kind = match &checked_kind {
            CheckedNamedTypeKind::Record { fields } => {
                let mut names = BTreeSet::new();
                let mut lowered = Vec::with_capacity(fields.len());
                for field in fields {
                    let field_name =
                        normalize_component_identifier(&field.source_name).map_err(|_| {
                            ComponentInterfaceError::InvalidNamedType(source_type.clone())
                        })?;
                    if !names.insert(field_name.clone()) {
                        return Err(ComponentInterfaceError::NamedMemberCollision {
                            ty: source_type.clone(),
                            name: field_name,
                        });
                    }
                    lowered.push(ComponentRecordFieldIr {
                        source_index: field.source_index,
                        source_name: field.source_name.clone(),
                        name: field_name,
                        ty: self.lower(&field.final_type, position)?,
                    });
                }
                ComponentNamedTypeKind::Record { fields: lowered }
            }
            CheckedNamedTypeKind::Enum { variants } => {
                let mut names = BTreeSet::new();
                let mut enum_cases = Vec::with_capacity(variants.len());
                let mut variant_cases = Vec::with_capacity(variants.len());
                let has_payload = variants
                    .iter()
                    .any(|variant| variant.payload_type.is_some());
                for variant in variants {
                    let case_name =
                        normalize_component_identifier(&variant.source_name).map_err(|_| {
                            ComponentInterfaceError::InvalidNamedType(source_type.clone())
                        })?;
                    if !names.insert(case_name.clone()) {
                        return Err(ComponentInterfaceError::NamedMemberCollision {
                            ty: source_type.clone(),
                            name: case_name,
                        });
                    }
                    if has_payload {
                        let payload = variant
                            .payload_type
                            .as_ref()
                            .map(|payload| self.lower(payload, position))
                            .transpose()?;
                        variant_cases.push(ComponentVariantCaseIr {
                            source_index: variant.source_index,
                            source_name: variant.source_name.clone(),
                            name: case_name,
                            payload,
                        });
                    } else {
                        enum_cases.push(ComponentEnumCaseIr {
                            source_index: variant.source_index,
                            source_name: variant.source_name.clone(),
                            name: case_name,
                        });
                    }
                }
                if has_payload {
                    ComponentNamedTypeKind::Variant {
                        cases: variant_cases,
                    }
                } else {
                    ComponentNamedTypeKind::Enum { cases: enum_cases }
                }
            }
        };
        self.visiting.remove(&source_type);
        self.declarations.insert(
            name.clone(),
            ComponentNamedTypeIr {
                name: name.clone(),
                source_type,
                kind,
            },
        );
        Ok(ComponentValueType::Named(name))
    }
}

fn component_named_type_name(final_type: &FinalType) -> Result<String, ComponentInterfaceError> {
    let TypedType::Record {
        name, type_args, ..
    } = final_type.as_typed_type()
    else {
        let TypedType::Enum { name } = final_type.as_typed_type() else {
            return Err(ComponentInterfaceError::InvalidNamedType(
                crate::type_checker::format_typed_type(final_type.as_typed_type()),
            ));
        };
        return normalize_component_identifier(name)
            .map_err(|_| ComponentInterfaceError::InvalidNamedType(name.clone()));
    };
    let base = normalize_component_identifier(name)
        .map_err(|_| ComponentInterfaceError::InvalidNamedType(name.clone()))?;
    if type_args.is_empty() {
        return Ok(base);
    }
    let mut identity = Vec::new();
    append_semantic_len(&mut identity, type_args.len());
    for type_arg in type_args {
        append_component_type_identity(&mut identity, type_arg)?;
    }
    Ok(format!("{base}-g-{}", hex_lower(&identity)))
}

fn append_component_type_identity(
    output: &mut Vec<u8>,
    ty: &TypedType,
) -> Result<(), ComponentInterfaceError> {
    match ty {
        TypedType::Unit => output.push(0),
        TypedType::Int32 => output.push(1),
        TypedType::Int64 => output.push(2),
        TypedType::Float64 => output.push(3),
        TypedType::Boolean => output.push(4),
        TypedType::String => output.push(5),
        TypedType::Char => output.push(6),
        TypedType::List(inner) => {
            output.push(7);
            append_component_type_identity(output, inner)?;
        }
        TypedType::Array(inner, ArrayLength::Known(length)) => {
            output.push(8);
            append_semantic_len(output, *length);
            append_component_type_identity(output, inner)?;
        }
        TypedType::Option(inner) => {
            output.push(9);
            append_component_type_identity(output, inner)?;
        }
        TypedType::Result(ok, err) => {
            output.push(10);
            append_component_type_identity(output, ok)?;
            append_component_type_identity(output, err)?;
        }
        TypedType::Record {
            name, type_args, ..
        } => {
            output.push(11);
            append_semantic_string(output, name);
            append_semantic_len(output, type_args.len());
            for type_arg in type_args {
                append_component_type_identity(output, type_arg)?;
            }
        }
        TypedType::Enum { name } => {
            output.push(12);
            append_semantic_string(output, name);
        }
        TypedType::Array(_, ArrayLength::AnyInternal)
        | TypedType::Function { .. }
        | TypedType::TypeParam(_)
        | TypedType::InferVar(_)
        | TypedType::Projection { .. }
        | TypedType::Temporal { .. } => {
            return Err(ComponentInterfaceError::InvalidNamedType(
                crate::type_checker::format_typed_type(ty),
            ));
        }
    }
    Ok(())
}

fn normalize_component_identifier(source: &str) -> Result<String, String> {
    let mut normalized = String::with_capacity(source.len());
    let mut previous_was_separator = false;
    for ch in source.chars() {
        match ch {
            'a'..='z' | '0'..='9' => {
                if normalized.is_empty() && ch.is_ascii_digit() {
                    return Err("the normalized name cannot start with a digit".to_string());
                }
                normalized.push(ch);
                previous_was_separator = false;
            }
            'A'..='Z' => {
                if !normalized.is_empty() && !previous_was_separator {
                    normalized.push('-');
                }
                normalized.push(ch.to_ascii_lowercase());
                previous_was_separator = false;
            }
            '_' => {
                if normalized.is_empty() || previous_was_separator {
                    return Err(
                        "leading or repeated underscores have no stable WIT spelling".to_string(),
                    );
                }
                normalized.push('-');
                previous_was_separator = true;
            }
            _ => {
                return Err(
                    "Component identifiers accept only ASCII letters, digits, and underscores"
                        .to_string(),
                );
            }
        }
    }
    if normalized.is_empty() || previous_was_separator {
        return Err("the normalized name is empty or ends with a separator".to_string());
    }
    wit_parser::validate_id(&normalized).map_err(|error| error.to_string())?;
    Ok(normalized)
}

fn write_wit_identifier(output: &mut String, identifier: &str) {
    if is_unescaped_wit_keyword(identifier) {
        output.push('%');
    }
    output.push_str(identifier);
}

fn emit_deterministic_wit(ir: &ComponentInterfaceIr) -> String {
    let mut output = String::new();
    output.push_str("package ");
    output.push_str(&ir.package);
    output.push_str(";\n");
    for interface in &ir.interfaces {
        output.push_str("\ninterface ");
        write_wit_identifier(&mut output, &interface.name);
        output.push_str(" {\n");
        for named_type in &interface.types {
            match &named_type.kind {
                ComponentNamedTypeKind::Record { fields } => {
                    output.push_str("  record ");
                    write_wit_identifier(&mut output, &named_type.name);
                    output.push_str(" {\n");
                    for field in fields {
                        output.push_str("    ");
                        write_wit_identifier(&mut output, &field.name);
                        output.push_str(": ");
                        field.ty.write_wit(&mut output);
                        output.push_str(",\n");
                    }
                    output.push_str("  }\n");
                }
                ComponentNamedTypeKind::Enum { cases } => {
                    output.push_str("  enum ");
                    write_wit_identifier(&mut output, &named_type.name);
                    output.push_str(" {\n");
                    for case in cases {
                        output.push_str("    ");
                        write_wit_identifier(&mut output, &case.name);
                        output.push_str(",\n");
                    }
                    output.push_str("  }\n");
                }
                ComponentNamedTypeKind::Variant { cases } => {
                    output.push_str("  variant ");
                    write_wit_identifier(&mut output, &named_type.name);
                    output.push_str(" {\n");
                    for case in cases {
                        output.push_str("    ");
                        write_wit_identifier(&mut output, &case.name);
                        if let Some(payload) = &case.payload {
                            output.push('(');
                            payload.write_wit(&mut output);
                            output.push(')');
                        }
                        output.push_str(",\n");
                    }
                    output.push_str("  }\n");
                }
            }
        }
        for resource in &interface.resources {
            output.push_str("  resource ");
            write_wit_identifier(&mut output, &resource.name);
            output.push_str(" {\n");
            for method in &resource.methods {
                output.push_str("    ");
                write_wit_identifier(&mut output, &method.name);
                match method.receiver {
                    ComponentResourceReceiverIr::Borrow => output.push_str(": func("),
                    ComponentResourceReceiverIr::Own => {
                        output.push_str(": static func(");
                        write_wit_identifier(&mut output, &method.receiver_name);
                        output.push_str(": own<");
                        write_wit_identifier(&mut output, &resource.name);
                        output.push('>');
                        if method
                            .params
                            .iter()
                            .any(|param| param.ty != ComponentValueType::Unit)
                        {
                            output.push_str(", ");
                        }
                    }
                }
                write_component_params(&mut output, &method.params);
                output.push(')');
                write_component_result(&mut output, &method.result);
                output.push_str(";\n");
            }
            output.push_str("  }\n\n");
        }
        for callable in &interface.callables {
            output.push_str("  resource ");
            write_wit_identifier(&mut output, &callable.name);
            output.push_str(" {\n    call: static func(callback: own<");
            write_wit_identifier(&mut output, &callable.name);
            output.push('>');
            if callable
                .params
                .iter()
                .any(|param| param.ty != ComponentValueType::Unit)
            {
                output.push_str(", ");
            }
            write_component_params(&mut output, &callable.params);
            output.push(')');
            write_component_result(&mut output, &callable.result);
            output.push_str(";\n  }\n\n");
        }
        for function in &interface.functions {
            output.push_str("  ");
            write_wit_identifier(&mut output, &function.name);
            output.push_str(": func(");
            write_component_params(&mut output, &function.params);
            output.push(')');
            write_component_result(&mut output, &function.result);
            output.push_str(";\n");
        }
        output.push_str("}\n");
    }
    output.push_str("\nworld ");
    write_wit_identifier(&mut output, &ir.world.name);
    output.push_str(" {\n");
    for interface in &ir.world.exported_interfaces {
        output.push_str("  export ");
        write_wit_identifier(&mut output, interface);
        output.push_str(";\n");
    }
    output.push_str("}\n");
    output
}

fn write_component_params(output: &mut String, params: &[ComponentParamIr]) {
    let mut first = true;
    for param in params {
        if param.ty == ComponentValueType::Unit {
            continue;
        }
        if !first {
            output.push_str(", ");
        }
        first = false;
        write_wit_identifier(output, &param.name);
        output.push_str(": ");
        param.ty.write_wit(output);
    }
}

fn write_component_result(output: &mut String, result: &ComponentValueType) {
    if result != &ComponentValueType::Unit {
        output.push_str(" -> ");
        result.write_wit(output);
    }
}

fn validate_generated_wit(wit: &str) -> Result<(), ComponentInterfaceError> {
    let mut resolve = wit_parser::Resolve::default();
    resolve
        .push_str("restrict-generated-component-interface.wit", wit)
        .map(|_| ())
        .map_err(|error| ComponentInterfaceError::GeneratedWitRejected(error.to_string()))
}

/// Project compiler-internal Component IR onto its stable semantic metadata.
///
/// Source symbols, parameter source indexes, spans, Core Wasm layouts, and
/// function bodies are intentionally excluded.
pub fn component_semantic_metadata(ir: &ComponentInterfaceIr) -> ComponentSemanticMetadata {
    ComponentSemanticMetadata {
        schema_version: COMPONENT_SEMANTIC_METADATA_SCHEMA_VERSION,
        implementation_subset: ir.implementation_subset.clone(),
        abi_profile: ir.abi_profile.clone(),
        package: ir.package.clone(),
        world: ir.world.name.clone(),
        interfaces: ir
            .interfaces
            .iter()
            .map(|interface| ComponentInterfaceMetadata {
                name: interface.name.clone(),
                types: interface
                    .types
                    .iter()
                    .map(|named_type| ComponentNamedTypeMetadata {
                        name: named_type.name.clone(),
                        kind: match &named_type.kind {
                            ComponentNamedTypeKind::Record { fields } => {
                                ComponentNamedTypeMetadataKind::Record {
                                    fields: fields
                                        .iter()
                                        .map(|field| ComponentRecordFieldMetadata {
                                            name: field.name.clone(),
                                            ty: field.ty.clone(),
                                        })
                                        .collect(),
                                }
                            }
                            ComponentNamedTypeKind::Enum { cases } => {
                                ComponentNamedTypeMetadataKind::Enum {
                                    cases: cases.iter().map(|case| case.name.clone()).collect(),
                                }
                            }
                            ComponentNamedTypeKind::Variant { cases } => {
                                ComponentNamedTypeMetadataKind::Variant {
                                    cases: cases
                                        .iter()
                                        .map(|case| ComponentVariantCaseMetadata {
                                            name: case.name.clone(),
                                            payload: case.payload.clone(),
                                        })
                                        .collect(),
                                }
                            }
                        },
                    })
                    .collect(),
                resources: interface
                    .resources
                    .iter()
                    .map(|resource| ComponentResourceMetadata {
                        name: resource.name.clone(),
                        methods: resource
                            .methods
                            .iter()
                            .map(|method| ComponentResourceMethodMetadata {
                                name: method.name.clone(),
                                receiver: method.receiver,
                                params: method
                                    .params
                                    .iter()
                                    .filter(|param| param.ty != ComponentValueType::Unit)
                                    .map(|param| ComponentParamMetadata {
                                        name: param.name.clone(),
                                        ty: param.ty.clone(),
                                    })
                                    .collect(),
                                result: method.result.clone(),
                            })
                            .collect(),
                    })
                    .collect(),
                callables: interface
                    .callables
                    .iter()
                    .map(|callable| ComponentCallableMetadata {
                        name: callable.name.clone(),
                        params: callable
                            .params
                            .iter()
                            .filter(|param| param.ty != ComponentValueType::Unit)
                            .map(|param| ComponentParamMetadata {
                                name: param.name.clone(),
                                ty: param.ty.clone(),
                            })
                            .collect(),
                        result: callable.result.clone(),
                        multiplicity: callable.multiplicity,
                    })
                    .collect(),
                functions: interface
                    .functions
                    .iter()
                    .map(|function| ComponentFunctionMetadata {
                        name: function.name.clone(),
                        params: function
                            .params
                            .iter()
                            .filter(|param| param.ty != ComponentValueType::Unit)
                            .map(|param| ComponentParamMetadata {
                                name: param.name.clone(),
                                ty: param.ty.clone(),
                            })
                            .collect(),
                        result: function.result.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// Compute the stable Component interface identity from normalized WIT and
/// stable ABI metadata.
///
/// Compatibility must still be classified with a semantic diff; equality of
/// this hash alone cannot explain whether a change is breaking.
pub fn component_interface_hash(
    normalized_wit: &str,
    metadata: &ComponentSemanticMetadata,
) -> [u8; 32] {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(b"restrict.component-interface.semantic\0");
    encoded.extend_from_slice(&metadata.schema_version.to_le_bytes());
    append_semantic_string(&mut encoded, normalized_wit);
    append_semantic_string(&mut encoded, &metadata.implementation_subset);
    append_semantic_string(&mut encoded, &metadata.abi_profile);
    append_semantic_string(&mut encoded, &metadata.package);
    append_semantic_string(&mut encoded, &metadata.world);
    append_semantic_len(&mut encoded, metadata.interfaces.len());
    for interface in &metadata.interfaces {
        append_semantic_string(&mut encoded, &interface.name);
        if !interface.types.is_empty() {
            encoded.extend_from_slice(b"\0restrict.named-types\0");
            append_semantic_len(&mut encoded, interface.types.len());
            for named_type in &interface.types {
                append_semantic_string(&mut encoded, &named_type.name);
                match &named_type.kind {
                    ComponentNamedTypeMetadataKind::Record { fields } => {
                        encoded.push(0);
                        append_semantic_len(&mut encoded, fields.len());
                        for field in fields {
                            append_semantic_string(&mut encoded, &field.name);
                            append_semantic_value_type(&mut encoded, &field.ty);
                        }
                    }
                    ComponentNamedTypeMetadataKind::Enum { cases } => {
                        encoded.push(1);
                        append_semantic_len(&mut encoded, cases.len());
                        for case in cases {
                            append_semantic_string(&mut encoded, case);
                        }
                    }
                    ComponentNamedTypeMetadataKind::Variant { cases } => {
                        encoded.push(2);
                        append_semantic_len(&mut encoded, cases.len());
                        for case in cases {
                            append_semantic_string(&mut encoded, &case.name);
                            match &case.payload {
                                Some(payload) => {
                                    encoded.push(1);
                                    append_semantic_value_type(&mut encoded, payload);
                                }
                                None => encoded.push(0),
                            }
                        }
                    }
                }
            }
        }
        if !interface.resources.is_empty() {
            encoded.extend_from_slice(b"\0restrict.resources\0");
            append_semantic_len(&mut encoded, interface.resources.len());
            for resource in &interface.resources {
                append_semantic_string(&mut encoded, &resource.name);
                append_semantic_len(&mut encoded, resource.methods.len());
                for method in &resource.methods {
                    append_semantic_string(&mut encoded, &method.name);
                    encoded.push(match method.receiver {
                        ComponentResourceReceiverIr::Borrow => 0,
                        ComponentResourceReceiverIr::Own => 1,
                    });
                    append_semantic_len(&mut encoded, method.params.len());
                    for param in &method.params {
                        append_semantic_string(&mut encoded, &param.name);
                        append_semantic_value_type(&mut encoded, &param.ty);
                    }
                    append_semantic_value_type(&mut encoded, &method.result);
                }
            }
        }
        if !interface.callables.is_empty() {
            encoded.extend_from_slice(b"\0restrict.callables\0");
            append_semantic_len(&mut encoded, interface.callables.len());
            for callable in &interface.callables {
                append_semantic_string(&mut encoded, &callable.name);
                encoded.push(match callable.multiplicity {
                    ComponentCallableMultiplicityIr::Once => 0,
                    ComponentCallableMultiplicityIr::Many => 1,
                });
                append_semantic_len(&mut encoded, callable.params.len());
                for param in &callable.params {
                    append_semantic_string(&mut encoded, &param.name);
                    append_semantic_value_type(&mut encoded, &param.ty);
                }
                append_semantic_value_type(&mut encoded, &callable.result);
            }
        }
        append_semantic_len(&mut encoded, interface.functions.len());
        for function in &interface.functions {
            append_semantic_string(&mut encoded, &function.name);
            append_semantic_len(&mut encoded, function.params.len());
            for param in &function.params {
                append_semantic_string(&mut encoded, &param.name);
                append_semantic_value_type(&mut encoded, &param.ty);
            }
            append_semantic_value_type(&mut encoded, &function.result);
        }
    }
    Sha256::digest(encoded).into()
}

fn append_semantic_value_type(encoded: &mut Vec<u8>, ty: &ComponentValueType) {
    encoded.push(ty.semantic_tag());
    match ty {
        ComponentValueType::List(element) | ComponentValueType::Option(element) => {
            append_semantic_value_type(encoded, element);
        }
        ComponentValueType::Array { element, length } => {
            append_semantic_len(encoded, *length);
            append_semantic_value_type(encoded, element);
        }
        ComponentValueType::Result { ok, err } => {
            append_semantic_value_type(encoded, ok);
            append_semantic_value_type(encoded, err);
        }
        ComponentValueType::Named(name)
        | ComponentValueType::OwnResource(name)
        | ComponentValueType::OwnCallable(name) => append_semantic_string(encoded, name),
        ComponentValueType::Unit
        | ComponentValueType::S32
        | ComponentValueType::S64
        | ComponentValueType::F64
        | ComponentValueType::Bool
        | ComponentValueType::Char
        | ComponentValueType::String => {}
    }
}

fn append_semantic_string(encoded: &mut Vec<u8>, value: &str) {
    append_semantic_len(encoded, value.len());
    encoded.extend_from_slice(value.as_bytes());
}

fn append_semantic_len(encoded: &mut Vec<u8>, value: usize) {
    encoded.extend_from_slice(&(value as u64).to_le_bytes());
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
#[derive(Debug, Error)]
enum ScalarAdapterError {
    #[error("generated WIT could not be resolved for adapter generation: {0}")]
    WitResolution(String),
    #[error("selected WIT world '{0}' was not found for adapter generation")]
    MissingWorld(String),
    #[error("generated WIT contains an unsupported world export while generating adapters")]
    UnsupportedWorldExport,
    #[error("generated WIT export '{interface}.{function}' has no matching ComponentInterfaceIr function")]
    MissingInterfaceFunction { interface: String, function: String },
    #[error(
        "ComponentInterfaceIr function '{interface}.{function}' was not found in generated WIT"
    )]
    MissingWitFunction { interface: String, function: String },
    #[error("Canonical ABI signature for '{interface}.{function}' is outside the scalar adapter subset: {reason}")]
    UnsupportedCanonicalSignature {
        interface: String,
        function: String,
        reason: String,
    },
    #[error("generated Core WAT has no final module delimiter")]
    MissingModuleDelimiter,
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn generate_scalar_adapter_core_wat(
    core_wat: &str,
    interface: &ComponentInterfaceArtifact,
    arena_addr: u32,
) -> Result<String, ScalarAdapterError> {
    use wit_parser::abi::{AbiVariant, WasmSignature, WasmType};
    use wit_parser::{ManglingAndAbi, WasmExport, WasmExportKind, WorldItem};

    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str(
            "restrict-generated-component-interface.wit",
            &interface.normalized_wit,
        )
        .map_err(|error| ScalarAdapterError::WitResolution(error.to_string()))?;
    let world = resolve
        .select_world(&[package], Some(&interface.ir.world.name))
        .map_err(|_| ScalarAdapterError::MissingWorld(interface.ir.world.name.clone()))?;

    let mut ir_functions = interface
        .ir
        .interfaces
        .iter()
        .flat_map(|interface| {
            interface
                .functions
                .iter()
                .map(move |function| ((interface.name.clone(), function.name.clone()), function))
        })
        .collect::<BTreeMap<_, _>>();
    let mut adapters = String::new();
    adapters.push_str("\n  ;; Canonical ABI adapters\n");
    adapters.push_str("  (global $__restrict_component_adapter_depth (mut i32) (i32.const 0))\n");
    let mut adapter_index = 0usize;

    for (world_key, item) in &resolve.worlds[world].exports {
        let WorldItem::Interface { id, .. } = item else {
            return Err(ScalarAdapterError::UnsupportedWorldExport);
        };
        let interface_name = resolve.interfaces[*id]
            .name
            .as_ref()
            .ok_or(ScalarAdapterError::UnsupportedWorldExport)?;
        for function in resolve.interfaces[*id].functions.values() {
            let lookup = (interface_name.clone(), function.name.clone());
            let function_ir = ir_functions.remove(&lookup).ok_or_else(|| {
                ScalarAdapterError::MissingInterfaceFunction {
                    interface: interface_name.clone(),
                    function: function.name.clone(),
                }
            })?;
            let signature = resolve.wasm_signature(AbiVariant::GuestExport, function);
            validate_scalar_adapter_signature(
                interface_name,
                &function.name,
                function_ir,
                &signature,
            )?;
            let canonical_name = resolve.wasm_export_name(
                ManglingAndAbi::Standard32,
                WasmExport::Func {
                    interface: Some(world_key),
                    func: function,
                    kind: WasmExportKind::Normal,
                },
            );
            emit_scalar_adapter(
                &mut adapters,
                adapter_index,
                &canonical_name,
                function_ir,
                &signature,
                arena_addr,
            );
            adapter_index += 1;
        }
    }

    if let Some(((interface, function), _)) = ir_functions.into_iter().next() {
        return Err(ScalarAdapterError::MissingWitFunction {
            interface,
            function,
        });
    }

    let module_end = core_wat
        .rfind(')')
        .ok_or(ScalarAdapterError::MissingModuleDelimiter)?;
    let mut adapted = String::with_capacity(core_wat.len() + adapters.len());
    adapted.push_str(&core_wat[..module_end]);
    adapted.push_str(&adapters);
    adapted.push_str(&core_wat[module_end..]);
    return Ok(adapted);

    fn validate_scalar_adapter_signature(
        interface: &str,
        function: &str,
        ir: &ComponentFunctionIr,
        signature: &WasmSignature,
    ) -> Result<(), ScalarAdapterError> {
        if ir.params.iter().any(|param| !param.ty.is_scalar_or_unit())
            || !ir.result.is_scalar_or_unit()
        {
            return Err(ScalarAdapterError::UnsupportedCanonicalSignature {
                interface: interface.to_string(),
                function: function.to_string(),
                reason: "rich values are outside the historical scalar regression adapter"
                    .to_string(),
            });
        }
        let expected_params = ir
            .params
            .iter()
            .filter_map(|param| scalar_canonical_wasm_type(&param.ty))
            .collect::<Vec<_>>();
        let expected_results = scalar_canonical_wasm_type(&ir.result)
            .into_iter()
            .collect::<Vec<_>>();
        if signature.indirect_params || signature.retptr {
            return Err(ScalarAdapterError::UnsupportedCanonicalSignature {
                interface: interface.to_string(),
                function: function.to_string(),
                reason: "indirect parameters or a return pointer are not scalar".to_string(),
            });
        }
        if signature.params != expected_params || signature.results != expected_results {
            return Err(ScalarAdapterError::UnsupportedCanonicalSignature {
                interface: interface.to_string(),
                function: function.to_string(),
                reason: format!(
                    "WIT oracle returned {:?} -> {:?}, expected {:?} -> {:?}",
                    signature.params, signature.results, expected_params, expected_results
                ),
            });
        }
        if signature
            .params
            .iter()
            .chain(&signature.results)
            .any(|ty| !matches!(ty, WasmType::I32 | WasmType::I64 | WasmType::F64))
        {
            return Err(ScalarAdapterError::UnsupportedCanonicalSignature {
                interface: interface.to_string(),
                function: function.to_string(),
                reason: "the scalar subset accepts only i32, i64, and f64 Core values".to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn scalar_canonical_wasm_type(ty: &ComponentValueType) -> Option<wit_parser::abi::WasmType> {
    use wit_parser::abi::WasmType;
    match ty {
        ComponentValueType::Unit => None,
        ComponentValueType::S32 | ComponentValueType::Bool | ComponentValueType::Char => {
            Some(WasmType::I32)
        }
        ComponentValueType::S64 => Some(WasmType::I64),
        ComponentValueType::F64 => Some(WasmType::F64),
        ComponentValueType::String
        | ComponentValueType::List(_)
        | ComponentValueType::Array { .. }
        | ComponentValueType::Option(_)
        | ComponentValueType::Result { .. }
        | ComponentValueType::Named(_)
        | ComponentValueType::OwnResource(_)
        | ComponentValueType::OwnCallable(_) => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn emit_scalar_adapter(
    output: &mut String,
    adapter_index: usize,
    canonical_name: &str,
    function: &ComponentFunctionIr,
    signature: &wit_parser::abi::WasmSignature,
    arena_addr: u32,
) {
    let adapter_name = format!("__restrict_component_adapter_{adapter_index}");
    output.push_str(&format!("  (func ${adapter_name}"));
    for (index, ty) in signature.params.iter().enumerate() {
        output.push_str(&format!(
            " (param $canonical_param_{index} {})",
            canonical_wat_type(*ty)
        ));
    }
    for ty in &signature.results {
        output.push_str(&format!(" (result {})", canonical_wat_type(*ty)));
    }
    output.push('\n');
    output.push_str("    (local $adapter_prev_arena i32)\n");
    output.push_str("    (local $adapter_entry_depth i32)\n");
    output.push_str("    (local $adapter_arena_mark i32)\n");
    if let Some(result) = signature.results.first() {
        output.push_str(&format!(
            "    (local $adapter_result {})\n",
            canonical_wat_type(*result)
        ));
    }
    output.push_str("    global.get $current_arena\n");
    output.push_str("    local.set $adapter_prev_arena\n");
    output.push_str("    global.get $__restrict_component_adapter_depth\n");
    output.push_str("    local.tee $adapter_entry_depth\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if\n");
    output.push_str("      (then\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        call $arena_init\n");
    output.push_str("        drop\n");
    output.push_str("      )\n");
    output.push_str("      (else\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        i32.const 4\n");
    output.push_str("        i32.add\n");
    output.push_str("        i32.load\n");
    output.push_str("        local.set $adapter_arena_mark\n");
    output.push_str("      )\n");
    output.push_str("    )\n");
    output.push_str("    local.get $adapter_entry_depth\n");
    output.push_str("    i32.const 1\n");
    output.push_str("    i32.add\n");
    output.push_str("    global.set $__restrict_component_adapter_depth\n");
    output.push_str(&format!("    i32.const {arena_addr}\n"));
    output.push_str("    global.set $current_arena\n");

    let mut canonical_index = 0usize;
    for param in &function.params {
        if param.ty == ComponentValueType::Unit {
            output.push_str("    i32.const 0\n");
        } else {
            output.push_str(&format!(
                "    local.get $canonical_param_{canonical_index}\n"
            ));
            canonical_index += 1;
        }
    }
    output.push_str(&format!("    call ${}\n", function.source_function));
    if !signature.results.is_empty() {
        output.push_str("    local.set $adapter_result\n");
    }

    output.push_str("    local.get $adapter_entry_depth\n");
    output.push_str("    global.set $__restrict_component_adapter_depth\n");
    output.push_str("    local.get $adapter_entry_depth\n");
    output.push_str("    i32.eqz\n");
    output.push_str("    (if\n");
    output.push_str("      (then\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        call $arena_reset\n");
    output.push_str("      )\n");
    output.push_str("      (else\n");
    output.push_str(&format!("        i32.const {arena_addr}\n"));
    output.push_str("        i32.const 4\n");
    output.push_str("        i32.add\n");
    output.push_str("        local.get $adapter_arena_mark\n");
    output.push_str("        i32.store\n");
    output.push_str("      )\n");
    output.push_str("    )\n");
    output.push_str("    local.get $adapter_prev_arena\n");
    output.push_str("    global.set $current_arena\n");
    if !signature.results.is_empty() {
        output.push_str("    local.get $adapter_result\n");
    }
    output.push_str("  )\n");
    output.push_str("  (export \"");
    append_wat_string_contents(output, canonical_name);
    output.push_str(&format!("\" (func ${adapter_name}))\n"));
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn canonical_wat_type(ty: wit_parser::abi::WasmType) -> &'static str {
    use wit_parser::abi::WasmType;
    match ty {
        WasmType::I32 | WasmType::Pointer | WasmType::Length => "i32",
        WasmType::I64 | WasmType::PointerOrI64 => "i64",
        WasmType::F32 => "f32",
        WasmType::F64 => "f64",
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
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

#[cfg(not(target_arch = "wasm32"))]
struct ProgramNamedTypeResolver<'a> {
    program: &'a Program,
    checker: &'a TypeChecker,
}

#[cfg(not(target_arch = "wasm32"))]
impl ComponentNamedTypeResolver for ProgramNamedTypeResolver<'_> {
    fn resolve_named_type(
        &self,
        name: &str,
        arguments: Vec<TypedType>,
    ) -> Result<TypedType, ComponentNamedTypeResolutionError> {
        for declaration in &self.program.declarations {
            let declaration = match declaration {
                TopDecl::Export(export) => export.item.as_ref(),
                declaration => declaration,
            };
            match declaration {
                TopDecl::Record(record) if record.name == name => {
                    if record
                        .type_params
                        .iter()
                        .any(|parameter| parameter.is_temporal)
                    {
                        return Err(ComponentNamedTypeResolutionError::Unsupported {
                            name: name.to_string(),
                            reason: "temporal record parameters are outside the synchronous Component contract"
                                .to_string(),
                        });
                    }
                    let expected = record.type_params.len();
                    if arguments.len() != expected {
                        return Err(ComponentNamedTypeResolutionError::Arity {
                            name: name.to_string(),
                            expected,
                            found: arguments.len(),
                        });
                    }
                    let ty = TypedType::Record {
                        name: name.to_string(),
                        type_args: arguments,
                        frozen: record.frozen,
                        hash: None,
                        parent_hash: record.parent_hash.clone(),
                    };
                    if self.checker.checked_record_fields_for_type(&ty).is_none() {
                        return Err(ComponentNamedTypeResolutionError::Unsupported {
                            name: name.to_string(),
                            reason: "record metadata could not be concretized".to_string(),
                        });
                    }
                    return Ok(ty);
                }
                TopDecl::Enum(enumeration) if enumeration.name == name => {
                    if !arguments.is_empty() {
                        return Err(ComponentNamedTypeResolutionError::Arity {
                            name: name.to_string(),
                            expected: 0,
                            found: arguments.len(),
                        });
                    }
                    return Ok(TypedType::Enum {
                        name: name.to_string(),
                    });
                }
                _ => {}
            }
        }
        Err(ComponentNamedTypeResolutionError::NotFound {
            name: name.to_string(),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_and_normalize_component_contract(
    selection: &ComponentSelectionContract,
) -> Result<ComponentSelectionContract, ComponentCompileError> {
    ComponentSelectionContract::try_new(
        selection.schema_version,
        selection.package.clone(),
        selection.world.clone(),
        selection.abi_profile.clone(),
        selection.exports.clone(),
        selection.resources.clone(),
        selection.callables.clone(),
    )
    .map_err(|error| ComponentCompileError::Contract(error.to_string()))
}

#[cfg(not(target_arch = "wasm32"))]
fn lower_component_specialization(
    source_function: &str,
    core_symbol: &str,
    external_name: &str,
    specialization: &crate::component_specialization::ComponentFunctionSpecialization,
    named_type_graph: &CheckedNamedTypeGraph,
) -> Result<(ComponentFunctionIr, Vec<ComponentNamedTypeIr>), ComponentCompileError> {
    let mut lowerer = ComponentTypeLowerer::new(source_function, named_type_graph);
    let mut params = Vec::with_capacity(specialization.params.len());
    let mut normalized_names = BTreeSet::new();
    for (source_index, (source_name, checked_ty)) in specialization.params.iter().enumerate() {
        let final_type = FinalType::new(checked_ty.clone())
            .map_err(|error| ComponentCompileError::Generic(format!("{error:?}")))?;
        let ty = lowerer
            .lower(&final_type, &format!("parameter '{source_name}'"))
            .map_err(ComponentCompileError::RootCatalog)?;
        let name = if ty == ComponentValueType::Unit {
            format!("unit-{source_index}")
        } else {
            normalize_component_identifier(source_name).map_err(|reason| {
                ComponentCompileError::RootCatalog(ComponentInterfaceError::InvalidParameterName {
                    function: source_function.to_string(),
                    parameter: source_name.clone(),
                    reason,
                })
            })?
        };
        if ty != ComponentValueType::Unit && !normalized_names.insert(name.clone()) {
            return Err(ComponentCompileError::RootCatalog(
                ComponentInterfaceError::ParameterNameCollision {
                    function: source_function.to_string(),
                    name,
                },
            ));
        }
        params.push(ComponentParamIr {
            source_index,
            source_name: source_name.clone(),
            name,
            ty,
        });
    }
    let result_type = FinalType::new(specialization.return_type.clone())
        .map_err(|error| ComponentCompileError::Generic(format!("{error:?}")))?;
    let result = lowerer
        .lower(&result_type, "return")
        .map_err(ComponentCompileError::RootCatalog)?;
    Ok((
        ComponentFunctionIr {
            source_function: core_symbol.to_string(),
            name: external_name.to_string(),
            params,
            result,
        },
        lowerer.finish(),
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn lower_planned_boundary_type(
    planned: &PlannedComponentBoundaryType,
    interface_name: &str,
    position: &str,
    lowerer: &mut ComponentTypeLowerer<'_>,
) -> Result<ComponentValueType, ComponentCompileError> {
    match planned {
        PlannedComponentBoundaryType::Value(ty) => {
            let final_type = FinalType::new(ty.clone())
                .map_err(|error| ComponentCompileError::Resource(format!("{error:?}")))?;
            lowerer
                .lower(&final_type, position)
                .map_err(ComponentCompileError::RootCatalog)
        }
        PlannedComponentBoundaryType::OwnResource(identity) => {
            if identity.interface != interface_name {
                return Err(ComponentCompileError::Resource(format!(
                    "{position}: cross-interface resource handle '{}.{}' is not supported in v0.1",
                    identity.interface, identity.external_name
                )));
            }
            Ok(ComponentValueType::OwnResource(
                identity.external_name.clone(),
            ))
        }
        PlannedComponentBoundaryType::OwnCallable(identity) => {
            if identity.interface != interface_name {
                return Err(ComponentCompileError::Resource(format!(
                    "{position}: cross-interface callable handle '{}.{}' is not supported in v0.1",
                    identity.interface, identity.external_name
                )));
            }
            Ok(ComponentValueType::OwnCallable(
                identity.external_name.clone(),
            ))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn lower_planned_params(
    source_function: &str,
    interface_name: &str,
    planned: &[PlannedComponentParam],
    lowerer: &mut ComponentTypeLowerer<'_>,
) -> Result<Vec<ComponentParamIr>, ComponentCompileError> {
    let mut params = Vec::with_capacity(planned.len());
    let mut names = BTreeSet::new();
    for param in planned {
        let ty = lower_planned_boundary_type(
            &param.ty,
            interface_name,
            &format!("parameter '{}'", param.source_name),
            lowerer,
        )?;
        let name = if ty == ComponentValueType::Unit {
            format!("unit-{}", param.source_index)
        } else {
            normalize_component_identifier(&param.source_name).map_err(|reason| {
                ComponentCompileError::RootCatalog(ComponentInterfaceError::InvalidParameterName {
                    function: source_function.to_string(),
                    parameter: param.source_name.clone(),
                    reason,
                })
            })?
        };
        if ty != ComponentValueType::Unit && !names.insert(name.clone()) {
            return Err(ComponentCompileError::RootCatalog(
                ComponentInterfaceError::ParameterNameCollision {
                    function: source_function.to_string(),
                    name,
                },
            ));
        }
        params.push(ComponentParamIr {
            source_index: param.source_index,
            source_name: param.source_name.clone(),
            name,
            ty,
        });
    }
    Ok(params)
}

#[cfg(not(target_arch = "wasm32"))]
fn merge_component_named_types(
    interface: &mut PendingComponentInterface,
    named_types: Vec<ComponentNamedTypeIr>,
) -> Result<(), ComponentCompileError> {
    for named_type in named_types {
        if let Some(existing) = interface.types.get(&named_type.name) {
            if existing != &named_type {
                return Err(ComponentCompileError::RootCatalog(
                    ComponentInterfaceError::NamedTypeCollision {
                        name: named_type.name,
                        first: existing.source_type.clone(),
                        second: named_type.source_type,
                    },
                ));
            }
        } else {
            interface.types.insert(named_type.name.clone(), named_type);
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn build_component_interface_contract(
    root_public: &RootPublicFunctionCatalog,
    resource_root: &ComponentResourceRootCatalog,
    resolved_program: &Program,
    checked_program: &CheckedProgramIr,
    checker: &TypeChecker,
    selection: &ComponentSelectionContract,
) -> Result<
    (
        ComponentInterfaceArtifact,
        ComponentGenericPlan,
        ComponentResourcePlan,
    ),
    ComponentCompileError,
> {
    let selection = validate_and_normalize_component_contract(selection)?;
    if selection.schema_version != COMPONENT_SELECTION_SCHEMA_V2 {
        return Err(ComponentCompileError::Contract(format!(
            "schema-v2 compilation requires schema {}, found {}",
            COMPONENT_SELECTION_SCHEMA_V2, selection.schema_version
        )));
    }
    if !checked_program.is_unmodified() || checked_program.validate_lowering_summaries().is_err() {
        return Err(ComponentCompileError::RootCatalog(
            ComponentInterfaceError::InvalidCheckedIr,
        ));
    }
    if !checked_program.matches_source_program(resolved_program) {
        return Err(ComponentCompileError::RootCatalog(
            ComponentInterfaceError::CheckedSourceProgramMismatch,
        ));
    }

    let resolver = ProgramNamedTypeResolver {
        program: resolved_program,
        checker,
    };
    let generic_plan = plan_component_generic_exports(&selection, checker, &resolver)
        .map_err(|error| ComponentCompileError::Generic(error.to_string()))?;
    let resource_plan =
        plan_component_resources(resource_root, resolved_program, checked_program, &selection)
            .map_err(|error| ComponentCompileError::Resource(error.to_string()))?;
    let specialization_by_identity = generic_plan
        .specializations
        .iter()
        .map(|specialization| (specialization.identity.as_str(), specialization))
        .collect::<BTreeMap<_, _>>();

    let mut interfaces = BTreeMap::<String, PendingComponentInterface>::new();
    for export in &resource_plan.exports {
        let mut lowerer =
            ComponentTypeLowerer::new(&export.source_function, &checked_program.named_types);
        let params = lower_planned_params(
            &export.source_function,
            &export.interface,
            &export.params,
            &mut lowerer,
        )?;
        let result =
            lower_planned_boundary_type(&export.result, &export.interface, "return", &mut lowerer)?;
        let function = ComponentFunctionIr {
            source_function: export.source_function.clone(),
            name: export.external_name.clone(),
            params,
            result,
        };
        let named_types = lowerer.finish();
        let interface = interfaces.entry(export.interface.clone()).or_default();
        merge_component_named_types(interface, named_types)?;
        if interface
            .functions
            .insert(export.external_name.clone(), function)
            .is_some()
        {
            return Err(ComponentCompileError::RootCatalog(
                ComponentInterfaceError::DuplicateExternalName(format!(
                    "{}.{}",
                    export.interface, export.external_name
                )),
            ));
        }
    }

    for resource in &resource_plan.resources {
        let mut methods = Vec::with_capacity(resource.methods.len());
        let mut all_named_types = Vec::new();
        for method in &resource.methods {
            let mut lowerer =
                ComponentTypeLowerer::new(&method.source_function, &checked_program.named_types);
            let params = lower_planned_params(
                &method.source_function,
                &resource.identity.interface,
                &method.params,
                &mut lowerer,
            )?;
            let result = lower_planned_boundary_type(
                &method.result,
                &resource.identity.interface,
                "return",
                &mut lowerer,
            )?;
            let receiver_name = normalize_component_identifier(&method.receiver_source_name)
                .map_err(|reason| {
                    ComponentCompileError::RootCatalog(
                        ComponentInterfaceError::InvalidParameterName {
                            function: method.source_function.clone(),
                            parameter: method.receiver_source_name.clone(),
                            reason,
                        },
                    )
                })?;
            methods.push(ComponentResourceMethodIr {
                source_function: method.source_function.clone(),
                name: method.external_name.clone(),
                receiver: match method.receiver {
                    crate::component_selection_v2::ComponentResourceReceiver::Borrow => {
                        ComponentResourceReceiverIr::Borrow
                    }
                    crate::component_selection_v2::ComponentResourceReceiver::Own => {
                        ComponentResourceReceiverIr::Own
                    }
                },
                receiver_name,
                params,
                result,
            });
            all_named_types.extend(lowerer.finish());
        }
        methods.sort_by(|left, right| left.name.cmp(&right.name));
        let interface = interfaces
            .entry(resource.identity.interface.clone())
            .or_default();
        merge_component_named_types(interface, all_named_types)?;
        let resource_ir = ComponentResourceIr {
            name: resource.identity.external_name.clone(),
            source_record: resource.identity.source_record.clone(),
            destructor_source_function: resource.destructor.source_function.clone(),
            methods,
        };
        if interface
            .resources
            .insert(resource_ir.name.clone(), resource_ir)
            .is_some()
        {
            return Err(ComponentCompileError::Resource(format!(
                "duplicate resource '{}.{}' reached Component IR",
                resource.identity.interface, resource.identity.external_name
            )));
        }
    }

    for callable in &resource_plan.callables {
        let synthetic_source = format!(
            "callable {}.{}",
            callable.identity.interface, callable.identity.external_name
        );
        let mut lowerer =
            ComponentTypeLowerer::new(&synthetic_source, &checked_program.named_types);
        let params = lower_planned_params(
            &synthetic_source,
            &callable.identity.interface,
            &callable.params,
            &mut lowerer,
        )?;
        let result = lower_planned_boundary_type(
            &callable.result,
            &callable.identity.interface,
            "return",
            &mut lowerer,
        )?;
        let named_types = lowerer.finish();
        let interface = interfaces
            .entry(callable.identity.interface.clone())
            .or_default();
        merge_component_named_types(interface, named_types)?;
        let callable_ir = ComponentCallableIr {
            name: callable.identity.external_name.clone(),
            params,
            result,
            multiplicity: ComponentCallableMultiplicityIr::Once,
        };
        if interface
            .callables
            .insert(callable_ir.name.clone(), callable_ir)
            .is_some()
        {
            return Err(ComponentCompileError::Resource(format!(
                "duplicate callable '{}.{}' reached Component IR",
                callable.identity.interface, callable.identity.external_name
            )));
        }
    }

    let checked_by_name = checked_program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    for export in &generic_plan.exports {
        let selected = selection
            .exports
            .iter()
            .find(|selected| {
                selected.interface == export.interface
                    && selected.external_name == export.external_name
                    && selected.source_function == export.source_function
            })
            .ok_or_else(|| {
                ComponentCompileError::Generic(format!(
                    "planned export '{}.{}' has no normalized selection",
                    export.interface, export.external_name
                ))
            })?;
        validate_generic_root_export(root_public, resolved_program, &checked_by_name, selected)?;
        let specialization = specialization_by_identity
            .get(export.specialization_identity.as_str())
            .ok_or_else(|| {
                ComponentCompileError::Generic(format!(
                    "planned specialization '{}' disappeared",
                    export.specialization_identity
                ))
            })?;
        let (function, named_types) = lower_component_specialization(
            &export.source_function,
            &export.core_symbol,
            &export.external_name,
            specialization,
            &checked_program.named_types,
        )?;
        let interface = interfaces.entry(export.interface.clone()).or_default();
        for named_type in named_types {
            if let Some(existing) = interface.types.get(&named_type.name) {
                if existing != &named_type {
                    return Err(ComponentCompileError::RootCatalog(
                        ComponentInterfaceError::NamedTypeCollision {
                            name: named_type.name,
                            first: existing.source_type.clone(),
                            second: named_type.source_type,
                        },
                    ));
                }
            } else {
                interface.types.insert(named_type.name.clone(), named_type);
            }
        }
        if interface
            .functions
            .insert(export.external_name.clone(), function)
            .is_some()
        {
            return Err(ComponentCompileError::RootCatalog(
                ComponentInterfaceError::DuplicateExternalName(format!(
                    "{}.{}",
                    export.interface, export.external_name
                )),
            ));
        }
    }

    let interfaces = interfaces
        .into_iter()
        .map(|(name, interface)| ComponentInterfaceDeclIr {
            name,
            types: interface.types.into_values().collect(),
            resources: interface.resources.into_values().collect(),
            callables: interface.callables.into_values().collect(),
            functions: interface.functions.into_values().collect(),
        })
        .collect::<Vec<_>>();
    let ir = ComponentInterfaceIr {
        schema_version: COMPONENT_INTERFACE_IR_SCHEMA_VERSION,
        implementation_subset: component_implementation_subset(&interfaces).to_string(),
        abi_profile: selection.abi_profile.clone(),
        package: selection.package.clone(),
        world: ComponentWorldIr {
            name: selection.world.clone(),
            exported_interfaces: interfaces
                .iter()
                .map(|interface| interface.name.clone())
                .collect(),
        },
        interfaces,
    };
    let normalized_wit = emit_deterministic_wit(&ir);
    validate_generated_wit(&normalized_wit)?;
    let metadata = component_semantic_metadata(&ir);
    let interface_hash = component_interface_hash(&normalized_wit, &metadata);
    Ok((
        ComponentInterfaceArtifact {
            ir,
            normalized_wit,
            metadata,
            interface_hash,
        },
        generic_plan,
        resource_plan,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_generic_root_export(
    root_public: &RootPublicFunctionCatalog,
    resolved_program: &Program,
    checked_by_name: &BTreeMap<&str, &CheckedFunctionIr>,
    selection: &ComponentFunctionExportContract,
) -> Result<(), ComponentCompileError> {
    let root = root_public
        .functions
        .get(&selection.source_function)
        .ok_or_else(|| {
            ComponentCompileError::RootCatalog(ComponentInterfaceError::NotRootPublicFunction(
                selection.source_function.clone(),
            ))
        })?;
    if !root_public.matches_resolved_root(&selection.source_function, resolved_program) {
        return Err(ComponentCompileError::RootCatalog(
            ComponentInterfaceError::ResolvedRootDeclarationMismatch(
                selection.source_function.clone(),
            ),
        ));
    }
    if root.declaration.is_async {
        return Err(ComponentCompileError::RootCatalog(
            ComponentInterfaceError::AsyncFunctionUnsupported(selection.source_function.clone()),
        ));
    }
    let checked = checked_by_name
        .get(selection.source_function.as_str())
        .ok_or_else(|| {
            ComponentCompileError::RootCatalog(ComponentInterfaceError::MissingCheckedFunction(
                selection.source_function.clone(),
            ))
        })?;
    if !checked.lowering.source_exported {
        return Err(ComponentCompileError::RootCatalog(
            ComponentInterfaceError::CheckedFunctionNotPublic(selection.source_function.clone()),
        ));
    }
    Ok(())
}

/// Compile one immutable Restrict source snapshot into a validated synchronous
/// WebAssembly Component artifact.
///
/// This is the single production seam shared by compiler tests and Warder. It
/// intentionally accepts source and paths rather than a prebuilt AST so every
/// published artifact is tied to one compiler-owned parse/resolve/check/lower
/// pipeline. Rich synchronous values are translated through compiler-owned
/// Canonical ABI adapters; async functions remain outside the v0.1 contract.
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_component(
    request: ComponentCompileRequest<'_>,
) -> Result<ComponentBuildArtifact, ComponentCompileError> {
    let (remaining, unresolved_root) = parse_program(request.source)
        .map_err(|error| ComponentCompileError::Parse(format!("{error:?}")))?;
    if !remaining.trim().is_empty() {
        return Err(ComponentCompileError::TrailingInput(
            remaining.chars().take(80).collect(),
        ));
    }

    let root_public = RootPublicFunctionCatalog::from_unresolved_root(&unresolved_root)?;
    let resolved_program = resolve_program_imports_for_file_with_package_roots(
        unresolved_root,
        request.entry_path,
        request.package_roots,
    )
    .map_err(|error| ComponentCompileError::ModuleResolution(error.to_string()))?;

    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved_program)
        .map_err(|error| ComponentCompileError::TypeCheck(error.to_string()))?;
    let checked_program = build_checked_ir(&resolved_program, &checker)
        .map_err(|error| ComponentCompileError::CheckedIr(error.to_string()))?;
    let interface = build_component_interface(
        &root_public,
        &resolved_program,
        &checked_program,
        request.selection,
    )?;
    let selected_functions = interface
        .ir
        .interfaces
        .iter()
        .flat_map(|interface| {
            interface
                .functions
                .iter()
                .map(|function| function.source_function.clone())
        })
        .collect::<Vec<_>>();
    let mut codegen = WasmCodeGen::new()
        .with_target_profile(WasmTargetProfile::WasmCore)
        .with_optimization_level(request.optimization_level)
        .with_component_internal_exports(selected_functions)
        .with_arena_size_bytes(request.arena_size_bytes)
        .map_err(|error| ComponentCompileError::CoreCodegen(error.to_string()))?;
    let core_wat = codegen
        .generate_checked(&resolved_program, &checked_program)
        .map_err(|error| ComponentCompileError::CoreCodegen(error.to_string()))?;
    let adapter_arena = codegen.component_adapter_arena_addr().ok_or_else(|| {
        ComponentCompileError::Adapter(
            "Component Core generation did not reserve its invocation arena".to_string(),
        )
    })?;
    let boundary_arena = codegen.component_boundary_arena_addr().ok_or_else(|| {
        ComponentCompileError::Adapter(
            "Component Core generation did not reserve its Canonical ABI boundary arena"
                .to_string(),
        )
    })?;
    let adapted_core_wat = generate_component_adapter_core_wat(
        &core_wat,
        &interface,
        adapter_arena,
        boundary_arena,
        request.arena_size_bytes,
    )
    .map_err(|error| ComponentCompileError::Adapter(error.to_string()))?;
    let packaged = package_canonical_component(&adapted_core_wat, &interface)?;

    Ok(ComponentBuildArtifact {
        component_wasm: packaged.component_wasm,
        normalized_wit: interface.normalized_wit,
        semantic_metadata: interface.metadata,
        interface_hash: interface.interface_hash,
        implementation_subset: interface.ir.implementation_subset,
        package: interface.ir.package,
        world: interface.ir.world.name,
        abi_profile: interface.ir.abi_profile,
    })
}

/// Compile a normalized schema-v2 synchronous Component contract.
///
/// Generic functions are accepted only through explicit, closed `type-args`.
/// Every requested instance is validated by the type checker, emitted once,
/// and exposed to Canonical ABI adapters under its stable planner-owned Core
/// symbol. No call-site inference can expand this public ABI.
#[cfg(not(target_arch = "wasm32"))]
pub fn compile_component_contract(
    request: ComponentCompileContractRequest<'_>,
) -> Result<ComponentBuildArtifact, ComponentCompileError> {
    let (remaining, unresolved_root) = parse_program(request.source)
        .map_err(|error| ComponentCompileError::Parse(format!("{error:?}")))?;
    if !remaining.trim().is_empty() {
        return Err(ComponentCompileError::TrailingInput(
            remaining.chars().take(80).collect(),
        ));
    }

    let root_public = RootPublicFunctionCatalog::from_unresolved_root(&unresolved_root)?;
    let resource_root = ComponentResourceRootCatalog::from_unresolved_root(&unresolved_root)
        .map_err(|error| ComponentCompileError::Resource(error.to_string()))?;
    let resolved_program = resolve_program_imports_for_file_with_package_roots(
        unresolved_root,
        request.entry_path,
        request.package_roots,
    )
    .map_err(|error| ComponentCompileError::ModuleResolution(error.to_string()))?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved_program)
        .map_err(|error| ComponentCompileError::TypeCheck(error.to_string()))?;
    let checked_program = build_checked_ir(&resolved_program, &checker)
        .map_err(|error| ComponentCompileError::CheckedIr(error.to_string()))?;
    let (interface, generic_plan, resource_plan) = build_component_interface_contract(
        &root_public,
        &resource_root,
        &resolved_program,
        &checked_program,
        &checker,
        request.selection,
    )?;

    let generic_symbols = generic_plan
        .specializations
        .iter()
        .map(|specialization| specialization.core_symbol.as_str())
        .collect::<BTreeSet<_>>();
    let mut selected_public_functions = interface
        .ir
        .interfaces
        .iter()
        .flat_map(|interface| interface.functions.iter())
        .filter(|function| !generic_symbols.contains(function.source_function.as_str()))
        .map(|function| function.source_function.clone())
        .collect::<BTreeSet<_>>();
    let mut selected_private_functions = BTreeSet::new();
    for resource in &resource_plan.resources {
        match resource.destructor.visibility {
            ComponentSourceVisibility::Public => {
                selected_public_functions.insert(resource.destructor.source_function.clone());
            }
            ComponentSourceVisibility::Private => {
                selected_private_functions.insert(resource.destructor.source_function.clone());
            }
        }
        for method in &resource.methods {
            match method.visibility {
                ComponentSourceVisibility::Public => {
                    selected_public_functions.insert(method.source_function.clone());
                }
                ComponentSourceVisibility::Private => {
                    selected_private_functions.insert(method.source_function.clone());
                }
            }
        }
    }
    let callable_factories = resource_plan
        .exports
        .iter()
        .filter(|export| matches!(export.result, PlannedComponentBoundaryType::OwnCallable(_)))
        .map(|export| export.source_function.clone())
        .collect::<BTreeSet<_>>();
    let resource_source_records = resource_plan
        .resources
        .iter()
        .map(|resource| resource.identity.source_record.clone())
        .collect::<BTreeSet<_>>();
    let mut codegen = WasmCodeGen::new()
        .with_target_profile(WasmTargetProfile::WasmCore)
        .with_optimization_level(request.optimization_level)
        .with_component_internal_exports(selected_public_functions)
        .with_component_private_internal_exports(selected_private_functions)
        .with_component_callable_capture_guard(callable_factories, resource_source_records)
        .with_component_generic_specializations(generic_plan.specializations)
        .with_arena_size_bytes(request.arena_size_bytes)
        .map_err(|error| ComponentCompileError::CoreCodegen(error.to_string()))?;
    let core_wat = codegen
        .generate_checked(&resolved_program, &checked_program)
        .map_err(|error| ComponentCompileError::CoreCodegen(error.to_string()))?;
    let adapter_arena = codegen.component_adapter_arena_addr().ok_or_else(|| {
        ComponentCompileError::Adapter(
            "Component Core generation did not reserve its invocation arena".to_string(),
        )
    })?;
    let boundary_arena = codegen.component_boundary_arena_addr().ok_or_else(|| {
        ComponentCompileError::Adapter(
            "Component Core generation did not reserve its Canonical ABI boundary arena"
                .to_string(),
        )
    })?;
    let resource_arena = codegen.component_resource_arena_addr().ok_or_else(|| {
        ComponentCompileError::Adapter(
            "Component Core generation did not reserve its resource/callable arena".to_string(),
        )
    })?;
    let adapted_core_wat = generate_component_adapter_core_wat(
        &core_wat,
        &interface,
        adapter_arena,
        boundary_arena,
        request.arena_size_bytes,
    )
    .map_err(|error| ComponentCompileError::Adapter(error.to_string()))?;
    let adapted_core_wat =
        if resource_plan.resources.is_empty() && resource_plan.callables.is_empty() {
            adapted_core_wat
        } else {
            generate_component_resource_adapter_core_wat(
                &adapted_core_wat,
                &interface,
                &resource_plan,
                ComponentResourceAdapterLayout {
                    invocation_arena_addr: adapter_arena,
                    resource_arena_addr: resource_arena,
                },
            )
            .map_err(|error| ComponentCompileError::Adapter(error.to_string()))?
        };
    let packaged = package_canonical_component(&adapted_core_wat, &interface)?;

    Ok(ComponentBuildArtifact {
        component_wasm: packaged.component_wasm,
        normalized_wit: interface.normalized_wit,
        semantic_metadata: interface.metadata,
        interface_hash: interface.interface_hash,
        implementation_subset: interface.ir.implementation_subset,
        package: interface.ir.package,
        world: interface.ir.world.name,
        abi_profile: interface.ir.abi_profile,
    })
}

/// Historical native proof that the scalar interface can be attached to real
/// Restrict Core Wasm and encoded as a validated Component.
///
/// This is deliberately not wired to Warder's production publication path;
/// it remains for scalar alias/metadata regression coverage.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Error)]
pub enum ScalarComponentPackagingError {
    #[error("Component interface artifact is internally inconsistent: {0}")]
    InvalidInterfaceArtifact(String),
    #[error("generated WIT could not be resolved for Component packaging: {0}")]
    WitResolution(String),
    #[error("selected WIT world '{0}' was not found")]
    MissingWorld(String),
    #[error("generated WIT contains an unsupported world export while packaging the scalar proof")]
    UnsupportedWorldExport,
    #[error("generated WIT export '{interface}.{function}' has no matching ComponentInterfaceIr function")]
    MissingInterfaceFunction { interface: String, function: String },
    #[error(
        "ComponentInterfaceIr function '{interface}.{function}' was not found in generated WIT"
    )]
    MissingWitFunction { interface: String, function: String },
    #[error("Core WAT does not expose selected source function '{0}' through the scalar Core ABI")]
    MissingCoreSourceExport(String),
    #[error("Core export '{0}' selected for the Component boundary is not a function")]
    CoreSourceExportNotFunction(String),
    #[error("the scalar Component proof requires an import-free Core module")]
    CoreImportsUnsupported,
    #[error("generated Component Core imports do not match its WIT resource intrinsics: {0}")]
    CoreResourceImportsMismatch(String),
    #[error("the scalar Component proof requires Core Wasm input, not a Component")]
    CoreModuleRequired,
    #[error("selected function '{0}' has a Unit parameter and requires an adapter wrapper")]
    UnitParameterRequiresAdapter(String),
    #[error("selected function '{interface}.{function}' has a rich Component boundary and requires Canonical ABI lift/lower adapters")]
    RichBoundaryRequiresAdapter { interface: String, function: String },
    #[error("Core WAT could not be encoded or validated: {0}")]
    CoreWatRejected(String),
    #[error("Core Wasm could not be re-encoded with Canonical ABI aliases: {0}")]
    CoreAliasEncoding(String),
    #[error("Component type metadata could not be embedded: {0}")]
    MetadataEmbedding(String),
    #[error("WebAssembly Component encoding failed: {0}")]
    ComponentEncoding(String),
    #[error("WebAssembly Component validation failed: {0}")]
    ComponentValidation(String),
    #[error("encoded Component does not conform to its generated WIT world: {0}")]
    ComponentTargetMismatch(String),
}

/// Package an import-free scalar Restrict Core WAT module as a Component proof.
///
/// The Core module must have been generated for the `wasm-core` target and
/// must already export every selected source function. Standard32 aliases are
/// added without changing the existing Core ABI exports. Unsupported shapes
/// have already been rejected while building `interface`.
#[cfg(not(target_arch = "wasm32"))]
pub fn experimental_package_scalar_component(
    core_wat: &str,
    interface: &ComponentInterfaceArtifact,
) -> Result<ExperimentalScalarComponentArtifact, ScalarComponentPackagingError> {
    use wit_component::{ComponentEncoder, StringEncoding};
    use wit_parser::{ManglingAndAbi, WasmExport, WasmExportKind, WorldItem};

    validate_interface_artifact_for_packaging(interface, false, false)?;

    let core = validate_import_free_core_wat(core_wat)?;

    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str(
            "restrict-generated-component-interface.wit",
            &interface.normalized_wit,
        )
        .map_err(|error| ScalarComponentPackagingError::WitResolution(error.to_string()))?;
    let world = resolve
        .select_world(&[package], Some(&interface.ir.world.name))
        .map_err(|_| {
            ScalarComponentPackagingError::MissingWorld(interface.ir.world.name.clone())
        })?;

    let mut source_by_wit_name = BTreeMap::new();
    for interface_ir in &interface.ir.interfaces {
        for function in &interface_ir.functions {
            source_by_wit_name.insert(
                (interface_ir.name.clone(), function.name.clone()),
                function.source_function.clone(),
            );
        }
    }

    let mut aliases = Vec::new();
    for (world_key, item) in &resolve.worlds[world].exports {
        let WorldItem::Interface { id, .. } = item else {
            return Err(ScalarComponentPackagingError::UnsupportedWorldExport);
        };
        let interface_name = resolve.interfaces[*id]
            .name
            .as_ref()
            .ok_or(ScalarComponentPackagingError::UnsupportedWorldExport)?;
        for function in resolve.interfaces[*id].functions.values() {
            let lookup = (interface_name.clone(), function.name.clone());
            let source_function = source_by_wit_name.remove(&lookup).ok_or_else(|| {
                ScalarComponentPackagingError::MissingInterfaceFunction {
                    interface: interface_name.clone(),
                    function: function.name.clone(),
                }
            })?;
            let canonical_name = resolve.wasm_export_name(
                ManglingAndAbi::Standard32,
                WasmExport::Func {
                    interface: Some(world_key),
                    func: function,
                    kind: WasmExportKind::Normal,
                },
            );
            aliases.push((source_function, canonical_name));
        }
    }
    if let Some(((interface, function), _)) = source_by_wit_name.into_iter().next() {
        return Err(ScalarComponentPackagingError::MissingWitFunction {
            interface,
            function,
        });
    }

    let indexed_aliases = aliases
        .into_iter()
        .map(|(source_function, canonical_name)| {
            let (kind, index) = core.exports.get(&source_function).ok_or_else(|| {
                ScalarComponentPackagingError::MissingCoreSourceExport(source_function.clone())
            })?;
            if *kind != wasmparser::ExternalKind::Func {
                return Err(ScalarComponentPackagingError::CoreSourceExportNotFunction(
                    source_function,
                ));
            }
            Ok((*index, canonical_name))
        })
        .collect::<Result<Vec<_>, ScalarComponentPackagingError>>()?;
    let aliased_core_wasm = add_standard32_export_aliases(&core.wasm, &indexed_aliases)?;
    wasmparser::Validator::new()
        .validate_all(&aliased_core_wasm)
        .map_err(|error| ScalarComponentPackagingError::CoreWatRejected(error.to_string()))?;
    let mut embedded_core_wasm = aliased_core_wasm.clone();

    wit_component::embed_component_metadata(
        &mut embedded_core_wasm,
        &resolve,
        world,
        StringEncoding::UTF8,
    )
    .map_err(|error| ScalarComponentPackagingError::MetadataEmbedding(error.to_string()))?;

    let component_wasm = ComponentEncoder::default()
        .module(&embedded_core_wasm)
        .map_err(|error| ScalarComponentPackagingError::ComponentEncoding(error.to_string()))?
        .reject_legacy_names(true)
        .validate(true)
        .encode()
        .map_err(|error| ScalarComponentPackagingError::ComponentEncoding(error.to_string()))?;

    wasmparser::Validator::new_with_features(
        wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
    )
    .validate_all(&component_wasm)
    .map_err(|error| ScalarComponentPackagingError::ComponentValidation(error.to_string()))?;
    wit_component::targets(&resolve, world, &component_wasm).map_err(|error| {
        ScalarComponentPackagingError::ComponentTargetMismatch(error.to_string())
    })?;

    Ok(ExperimentalScalarComponentArtifact {
        aliased_core_wasm,
        embedded_core_wasm,
        component_wasm,
    })
}

/// Package a Core module that already exports generated Standard32 Canonical
/// ABI adapters. Unlike the historical alias proof, this path supports source
/// `Unit` parameters because their dummy internal value is synthesized by the
/// wrapper and does not appear in WIT.
#[cfg(not(target_arch = "wasm32"))]
fn package_canonical_component(
    adapted_core_wat: &str,
    interface: &ComponentInterfaceArtifact,
) -> Result<ExperimentalScalarComponentArtifact, ScalarComponentPackagingError> {
    use wit_component::{ComponentEncoder, StringEncoding};

    validate_interface_artifact_for_packaging(interface, true, true)?;
    let mut resolve = wit_parser::Resolve::default();
    let package = resolve
        .push_str(
            "restrict-generated-component-interface.wit",
            &interface.normalized_wit,
        )
        .map_err(|error| ScalarComponentPackagingError::WitResolution(error.to_string()))?;
    let world = resolve
        .select_world(&[package], Some(&interface.ir.world.name))
        .map_err(|_| {
            ScalarComponentPackagingError::MissingWorld(interface.ir.world.name.clone())
        })?;
    let allowed_imports = canonical_resource_intrinsic_imports(&resolve, world)?;
    let core = validate_canonical_core_wat(adapted_core_wat, allowed_imports)?;

    let aliased_core_wasm = core.wasm;
    let mut embedded_core_wasm = aliased_core_wasm.clone();
    wit_component::embed_component_metadata(
        &mut embedded_core_wasm,
        &resolve,
        world,
        StringEncoding::UTF8,
    )
    .map_err(|error| ScalarComponentPackagingError::MetadataEmbedding(error.to_string()))?;

    let component_wasm = ComponentEncoder::default()
        .module(&embedded_core_wasm)
        .map_err(|error| ScalarComponentPackagingError::ComponentEncoding(error.to_string()))?
        .reject_legacy_names(true)
        .validate(true)
        .encode()
        .map_err(|error| ScalarComponentPackagingError::ComponentEncoding(error.to_string()))?;
    wasmparser::Validator::new_with_features(
        wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
    )
    .validate_all(&component_wasm)
    .map_err(|error| ScalarComponentPackagingError::ComponentValidation(error.to_string()))?;
    wit_component::targets(&resolve, world, &component_wasm).map_err(|error| {
        ScalarComponentPackagingError::ComponentTargetMismatch(error.to_string())
    })?;

    Ok(ExperimentalScalarComponentArtifact {
        aliased_core_wasm,
        embedded_core_wasm,
        component_wasm,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn canonical_resource_intrinsic_imports(
    resolve: &wit_parser::Resolve,
    world: wit_parser::WorldId,
) -> Result<BTreeSet<(String, String)>, ScalarComponentPackagingError> {
    use wit_parser::{ManglingAndAbi, ResourceIntrinsic, TypeDefKind, WasmImport, WorldItem};

    let mut imports = BTreeSet::new();
    for (world_key, item) in &resolve.worlds[world].exports {
        let WorldItem::Interface { id, .. } = item else {
            return Err(ScalarComponentPackagingError::UnsupportedWorldExport);
        };
        for resource in resolve.interfaces[*id].types.values().copied() {
            if !matches!(resolve.types[resource].kind, TypeDefKind::Resource) {
                continue;
            }
            for intrinsic in [
                ResourceIntrinsic::ExportedNew,
                ResourceIntrinsic::ExportedRep,
                ResourceIntrinsic::ExportedDrop,
            ] {
                imports.insert(resolve.wasm_import_name(
                    ManglingAndAbi::Standard32,
                    WasmImport::ResourceIntrinsic {
                        interface: Some(world_key),
                        resource,
                        intrinsic,
                    },
                ));
            }
        }
    }
    Ok(imports)
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_canonical_core_wat(
    core_wat: &str,
    mut allowed_imports: BTreeSet<(String, String)>,
) -> Result<ValidatedCoreModule, ScalarComponentPackagingError> {
    use wasmparser::{Encoding, Parser, Payload, TypeRef};

    let wasm = wat::parse_str(core_wat)
        .map_err(|error| ScalarComponentPackagingError::CoreWatRejected(error.to_string()))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|error| ScalarComponentPackagingError::CoreWatRejected(error.to_string()))?;

    let mut exports = BTreeMap::new();
    for payload in Parser::new(0).parse_all(&wasm) {
        match payload
            .map_err(|error| ScalarComponentPackagingError::CoreWatRejected(error.to_string()))?
        {
            Payload::Version { encoding, .. } if encoding != Encoding::Module => {
                return Err(ScalarComponentPackagingError::CoreModuleRequired);
            }
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.map_err(|error| {
                        ScalarComponentPackagingError::CoreWatRejected(error.to_string())
                    })?;
                    if !matches!(import.ty, TypeRef::Func(_) | TypeRef::FuncExact(_)) {
                        return Err(ScalarComponentPackagingError::CoreResourceImportsMismatch(
                            format!(
                                "non-function import '{}.{}' is not a Canonical resource intrinsic",
                                import.module, import.name
                            ),
                        ));
                    }
                    let identity = (import.module.to_string(), import.name.to_string());
                    if !allowed_imports.remove(&identity) {
                        return Err(ScalarComponentPackagingError::CoreResourceImportsMismatch(
                            format!(
                                "unexpected or duplicate import '{}.{}'",
                                import.module, import.name
                            ),
                        ));
                    }
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.map_err(|error| {
                        ScalarComponentPackagingError::CoreWatRejected(error.to_string())
                    })?;
                    exports.insert(export.name.to_string(), (export.kind, export.index));
                }
            }
            _ => {}
        }
    }
    if !allowed_imports.is_empty() {
        let missing = allowed_imports
            .into_iter()
            .map(|(module, name)| format!("{module}.{name}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(ScalarComponentPackagingError::CoreResourceImportsMismatch(
            format!("missing intrinsic import(s): {missing}"),
        ));
    }
    Ok(ValidatedCoreModule { wasm, exports })
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_interface_artifact_for_packaging(
    interface: &ComponentInterfaceArtifact,
    allow_unit_params: bool,
    allow_rich: bool,
) -> Result<(), ScalarComponentPackagingError> {
    if interface.ir.schema_version != COMPONENT_INTERFACE_IR_SCHEMA_VERSION {
        return Err(ScalarComponentPackagingError::InvalidInterfaceArtifact(
            "unsupported ComponentInterfaceIr schema version".to_string(),
        ));
    }
    let expected_subset = component_implementation_subset(&interface.ir.interfaces);
    if interface.ir.implementation_subset != expected_subset {
        return Err(ScalarComponentPackagingError::InvalidInterfaceArtifact(
            "implementation subset does not match ComponentInterfaceIr value kinds".to_string(),
        ));
    }
    if interface.ir.abi_profile != COMPONENT_ABI_PROFILE {
        return Err(ScalarComponentPackagingError::InvalidInterfaceArtifact(
            "unexpected ABI profile".to_string(),
        ));
    }
    if interface.normalized_wit != emit_deterministic_wit(&interface.ir) {
        return Err(ScalarComponentPackagingError::InvalidInterfaceArtifact(
            "normalized WIT does not match ComponentInterfaceIr".to_string(),
        ));
    }
    let metadata = component_semantic_metadata(&interface.ir);
    if interface.metadata != metadata
        || interface.interface_hash
            != component_interface_hash(&interface.normalized_wit, &metadata)
    {
        return Err(ScalarComponentPackagingError::InvalidInterfaceArtifact(
            "semantic metadata or interface hash does not match ComponentInterfaceIr".to_string(),
        ));
    }
    if !allow_rich {
        if let Some((interface_name, function_name)) = first_rich_boundary(&interface.ir) {
            return Err(ScalarComponentPackagingError::RichBoundaryRequiresAdapter {
                interface: interface_name.to_string(),
                function: function_name.to_string(),
            });
        }
    }
    for interface_ir in &interface.ir.interfaces {
        for function in &interface_ir.functions {
            if !allow_unit_params
                && function
                    .params
                    .iter()
                    .any(|param| param.ty == ComponentValueType::Unit)
            {
                return Err(ScalarComponentPackagingError::UnitParameterRequiresAdapter(
                    function.source_function.clone(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_import_free_core_wat(
    core_wat: &str,
) -> Result<ValidatedCoreModule, ScalarComponentPackagingError> {
    use wasmparser::{Encoding, Parser, Payload};

    let wasm = wat::parse_str(core_wat)
        .map_err(|error| ScalarComponentPackagingError::CoreWatRejected(error.to_string()))?;
    wasmparser::Validator::new()
        .validate_all(&wasm)
        .map_err(|error| ScalarComponentPackagingError::CoreWatRejected(error.to_string()))?;

    let mut exports = BTreeMap::new();
    for payload in Parser::new(0).parse_all(&wasm) {
        match payload
            .map_err(|error| ScalarComponentPackagingError::CoreWatRejected(error.to_string()))?
        {
            Payload::Version { encoding, .. } if encoding != Encoding::Module => {
                return Err(ScalarComponentPackagingError::CoreModuleRequired);
            }
            Payload::ImportSection(section) if section.count() != 0 => {
                return Err(ScalarComponentPackagingError::CoreImportsUnsupported);
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.map_err(|error| {
                        ScalarComponentPackagingError::CoreWatRejected(error.to_string())
                    })?;
                    exports.insert(export.name.to_string(), (export.kind, export.index));
                }
            }
            _ => {}
        }
    }
    Ok(ValidatedCoreModule { wasm, exports })
}

#[cfg(not(target_arch = "wasm32"))]
fn add_standard32_export_aliases(
    core_wasm: &[u8],
    aliases: &[(u32, String)],
) -> Result<Vec<u8>, ScalarComponentPackagingError> {
    use std::convert::Infallible;
    use wasm_encoder::reencode::{Error, Reencode};

    struct ExportAliasReencoder<'a> {
        aliases: &'a [(u32, String)],
    }

    impl Reencode for ExportAliasReencoder<'_> {
        type Error = Infallible;

        fn parse_export_section(
            &mut self,
            exports: &mut wasm_encoder::ExportSection,
            section: wasmparser::ExportSectionReader<'_>,
        ) -> Result<(), Error<Self::Error>> {
            wasm_encoder::reencode::utils::parse_export_section(self, exports, section)?;
            for (function_index, canonical_name) in self.aliases {
                exports.export(
                    canonical_name,
                    wasm_encoder::ExportKind::Func,
                    *function_index,
                );
            }
            Ok(())
        }
    }

    let mut module = wasm_encoder::Module::new();
    ExportAliasReencoder { aliases }
        .parse_core_module(&mut module, wasmparser::Parser::new(0), core_wasm)
        .map_err(|error| ScalarComponentPackagingError::CoreAliasEncoding(error.to_string()))?;
    Ok(module.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::builder::build_checked_ir;
    use crate::module::resolve_program_imports_with_module_source_map;
    use crate::{parse_program, TypeChecker};
    use std::collections::HashMap;

    fn parse(source: &str) -> Program {
        let (remaining, program) = parse_program(source).expect("source should parse");
        assert!(
            remaining.trim().is_empty(),
            "source should parse completely: {remaining:?}"
        );
        program
    }

    fn checked(program: &Program) -> CheckedProgramIr {
        let mut checker = TypeChecker::new();
        checker
            .check_program(program)
            .expect("source should type check");
        build_checked_ir(program, &checker).expect("checked IR should build")
    }

    fn artifact(
        source: &str,
        exports: Vec<ComponentExportSelection>,
    ) -> ComponentInterfaceArtifact {
        let root = parse(source);
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&root).unwrap();
        let checked = checked(&root);
        build_component_interface(
            &catalog,
            &root,
            &checked,
            &ComponentSelection::v1(
                "test:scalar@0.1.0",
                "scalar",
                COMPONENT_ABI_PROFILE,
                exports,
            ),
        )
        .unwrap()
    }

    #[test]
    fn scalar_interface_has_deterministic_valid_wit_golden() {
        let artifact = artifact(
            r#"
pub fun no_args: () -> () = {
    ()
}

pub fun keep_active: (
    left: Int32,
    total: Int64,
    ratio: Float64,
    active: Boolean,
    marker: Char
) -> Boolean = {
    active
}
"#,
            vec![
                ComponentExportSelection::new("api", "no_args", "no-args"),
                ComponentExportSelection::new("api", "keep_active", "keep-active"),
            ],
        );

        assert_eq!(
            artifact.normalized_wit,
            "package test:scalar@0.1.0;\n\ninterface api {\n  keep-active: func(left: s32, total: s64, ratio: f64, active: bool, marker: char) -> bool;\n  no-args: func();\n}\n\nworld scalar {\n  export api;\n}\n"
        );
        assert_eq!(
            artifact.interface_hash_hex(),
            "f9a273131ea02a8ae4fe1ee31efcfae3ed9f954f2f98e0af93fe930e568ab6c8"
        );
        assert_eq!(artifact.metadata.abi_profile, COMPONENT_ABI_PROFILE);
        assert_eq!(
            artifact.ir.implementation_subset,
            COMPONENT_SCALAR_IMPLEMENTATION_SUBSET
        );
        validate_generated_wit(&artifact.normalized_wit).unwrap();
    }

    #[test]
    fn parameter_names_normalize_and_wit_keywords_are_escaped() {
        let artifact = artifact(
            r#"
pub fun normalize: (
    user_id: Int32,
    camelCase: Int32,
    result: Int32
) -> Int32 = {
    result
}
"#,
            vec![ComponentExportSelection::new(
                "api",
                "normalize",
                "normalize",
            )],
        );

        assert!(artifact
            .normalized_wit
            .contains("normalize: func(user-id: s32, camel-case: s32, %result: s32) -> s32"));
        assert_eq!(
            artifact.metadata.interfaces[0].functions[0].params[0].name,
            "user-id"
        );
    }

    #[test]
    fn canonical_packaging_accepts_only_exact_resource_intrinsics() {
        let allowed = BTreeSet::from([("canonical-resource".to_string(), "item-new".to_string())]);
        validate_canonical_core_wat(
            r#"(module (import "canonical-resource" "item-new" (func)))"#,
            allowed.clone(),
        )
        .expect("the exact WIT-derived function import should be accepted");

        let unexpected = validate_canonical_core_wat(
            r#"(module (import "canonical-resource" "other" (func)))"#,
            allowed.clone(),
        )
        .unwrap_err();
        assert!(matches!(
            unexpected,
            ScalarComponentPackagingError::CoreResourceImportsMismatch(message)
                if message.contains("unexpected")
        ));

        let missing = validate_canonical_core_wat("(module)", allowed).unwrap_err();
        assert!(matches!(
            missing,
            ScalarComponentPackagingError::CoreResourceImportsMismatch(message)
                if message.contains("missing")
        ));
    }

    #[test]
    fn parameter_normalization_collisions_fail_closed() {
        let root = parse(
            r#"
pub fun collide: (user_id: Int32, userId: Int32) -> Int32 = {
    user_id
}
"#,
        );
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&root).unwrap();
        let checked = checked(&root);
        let error = build_component_interface(
            &catalog,
            &root,
            &checked,
            &ComponentSelection::v1(
                "test:param-collision@0.1.0",
                "param-collision",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new("api", "collide", "collide")],
            ),
        )
        .unwrap_err();
        assert_eq!(
            error,
            ComponentInterfaceError::ParameterNameCollision {
                function: "collide".to_string(),
                name: "user-id".to_string(),
            }
        );
    }

    #[test]
    fn pub_and_explicit_selection_are_both_required() {
        let root = parse(
            r#"
fun private_value: () -> Int32 = { 1 }
pub fun public_value: () -> Int32 = { 2 }
pub fun unselected_value: () -> Int32 = { 3 }
"#,
        );
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&root).unwrap();
        let checked = checked(&root);

        let private_error = build_component_interface(
            &catalog,
            &root,
            &checked,
            &ComponentSelection::v1(
                "test:gate@0.1.0",
                "gate",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new(
                    "api",
                    "private_value",
                    "private-value",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(
            private_error,
            ComponentInterfaceError::NotRootPublicFunction("private_value".to_string())
        );

        let selected = build_component_interface(
            &catalog,
            &root,
            &checked,
            &ComponentSelection::v1(
                "test:gate@0.1.0",
                "gate",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new(
                    "api",
                    "public_value",
                    "public-value",
                )],
            ),
        )
        .unwrap();
        assert!(selected
            .normalized_wit
            .contains("public-value: func() -> s32"));
        assert!(!selected.normalized_wit.contains("unselected-value"));
    }

    #[test]
    fn unit_parameter_and_result_carry_no_wit_value() {
        let artifact = artifact(
            "pub fun consume_unit: (value: ()) -> () = { () }",
            vec![ComponentExportSelection::new(
                "api",
                "consume_unit",
                "consume-unit",
            )],
        );

        assert!(artifact.normalized_wit.contains("consume-unit: func();"));
        let function = &artifact.metadata.interfaces[0].functions[0];
        assert!(function.params.is_empty());
        assert_eq!(function.result, ComponentValueType::Unit);
    }

    #[test]
    fn catalog_and_checked_source_must_match() {
        let catalog_root = parse("pub fun answer: () -> Int32 = { 1 }");
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&catalog_root).unwrap();
        let other_root = parse("pub fun answer: () -> Int64 = { 1 }");
        let other_checked = checked(&other_root);
        let selection = ComponentSelection::v1(
            "test:source-match@0.1.0",
            "source-match",
            COMPONENT_ABI_PROFILE,
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        assert_eq!(
            build_component_interface(&catalog, &other_root, &other_checked, &selection)
                .unwrap_err(),
            ComponentInterfaceError::ResolvedRootDeclarationMismatch("answer".to_string())
        );
        assert_eq!(
            build_component_interface(&catalog, &catalog_root, &other_checked, &selection)
                .unwrap_err(),
            ComponentInterfaceError::CheckedSourceProgramMismatch
        );

        let inferred_root = parse("pub fun inferred: () = { 1 }");
        let inferred_catalog =
            RootPublicFunctionCatalog::from_unresolved_root(&inferred_root).unwrap();
        let inferred_checked = checked(&inferred_root);
        let inferred = build_component_interface(
            &inferred_catalog,
            &inferred_root,
            &inferred_checked,
            &ComponentSelection::v1(
                "test:inferred@0.1.0",
                "inferred",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new("api", "inferred", "inferred")],
            ),
        )
        .unwrap();
        assert!(inferred.normalized_wit.contains("inferred: func() -> s32"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn actual_restrict_scalar_core_module_componentizes_and_validates() {
        use crate::{check_release_surface, HostAbiProfile, WasmCodeGen, WasmTargetProfile};

        let root = parse(
            r#"
pub fun increment: (value: Int32) -> Int32 = {
    value + 1
}

pub fun keep_active: (
    left: Int32,
    total: Int64,
    ratio: Float64,
    active: Boolean,
    marker: Char
) -> Boolean = {
    active
}

pub fun consume_value: (value: Int32) -> () = {
    ()
}
"#,
        );
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&root).unwrap();
        let mut checker = TypeChecker::new();
        checker.check_program(&root).unwrap();
        check_release_surface(&root, &checker, HostAbiProfile::V001Scalar).unwrap();
        let checked = build_checked_ir(&root, &checker).unwrap();
        let interface = build_component_interface(
            &catalog,
            &root,
            &checked,
            &ComponentSelection::v1(
                "test:component-proof@0.1.0",
                "component-proof",
                COMPONENT_ABI_PROFILE,
                vec![
                    ComponentExportSelection::new("api", "increment", "increment"),
                    ComponentExportSelection::new("api", "keep_active", "keep-active"),
                    ComponentExportSelection::new("api", "consume_value", "consume-value"),
                ],
            ),
        )
        .unwrap();
        let mut codegen = WasmCodeGen::with_host_abi_profile(HostAbiProfile::V001Scalar)
            .with_target_profile(WasmTargetProfile::WasmCore);
        let core_wat = codegen.generate_checked(&root, &checked).unwrap();
        assert!(!core_wat.contains("wasi_snapshot_preview1"));

        let packaged = experimental_package_scalar_component(&core_wat, &interface).unwrap();
        let packaged_again = experimental_package_scalar_component(&core_wat, &interface).unwrap();
        assert_eq!(packaged.aliased_core_wasm, packaged_again.aliased_core_wasm);
        assert_eq!(
            packaged.embedded_core_wasm,
            packaged_again.embedded_core_wasm
        );
        assert_eq!(packaged.component_wasm, packaged_again.component_wasm);
        assert!(wasmparser::Parser::is_component(&packaged.component_wasm));
        wasmparser::Validator::new_with_features(
            wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
        )
        .validate_all(&packaged.component_wasm)
        .unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn compiler_library_component_pipeline_is_deterministic_and_validated() {
        let snapshot = tempfile::tempdir().expect("component snapshot should be creatable");
        let entry_path = snapshot.path().join("main.rl");
        let source = r#"
pub fun answer: (value: Int32) -> Int32 = {
    value
}
"#;
        let selection = ComponentSelection::v1(
            "test:compiler-api@0.1.0",
            "compiler-api",
            COMPONENT_ABI_PROFILE,
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        let compile = || {
            compile_component(ComponentCompileRequest {
                source,
                entry_path: &entry_path,
                package_roots: &[],
                selection: &selection,
                optimization_level: WasmOptimizationLevel::Release,
                arena_size_bytes: 0x1000,
            })
            .expect("compiler-owned Component pipeline should succeed")
        };

        let first = compile();
        let second = compile();
        assert_eq!(first, second);
        assert!(wasmparser::Parser::is_component(&first.component_wasm));
        wasmparser::Validator::new_with_features(
            wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
        )
        .validate_all(&first.component_wasm)
        .expect("compiler API must return a validated Component");
        assert_eq!(first.interface_hash_hex().len(), 64);
        assert_eq!(first.package, "test:compiler-api@0.1.0");
        assert_eq!(first.world, "compiler-api");
        assert_eq!(first.abi_profile, COMPONENT_ABI_PROFILE);
        assert_eq!(
            first.implementation_subset,
            COMPONENT_SCALAR_IMPLEMENTATION_SUBSET
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn compiler_library_component_pipeline_preserves_typed_stage_errors() {
        let snapshot = tempfile::tempdir().expect("component snapshot should be creatable");
        let entry_path = snapshot.path().join("main.rl");
        let selection = ComponentSelection::v1(
            "test:compiler-api@0.1.0",
            "compiler-api",
            COMPONENT_ABI_PROFILE,
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        let error = compile_component(ComponentCompileRequest {
            source: "pub fun answer: () -> Missing = { () }",
            entry_path: &entry_path,
            package_roots: &[],
            selection: &selection,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect_err("unknown source types must fail before artifact publication");

        assert!(matches!(error, ComponentCompileError::TypeCheck(_)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn componentizer_rejects_core_module_without_selected_export() {
        let interface = artifact(
            "pub fun answer: () -> Int32 = { 1 }",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        let error = experimental_package_scalar_component(
            "(module (func $answer (result i32) i32.const 1))",
            &interface,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ScalarComponentPackagingError::MissingCoreSourceExport(source) if source == "answer"
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn componentizer_rejects_unit_parameter_until_adapter_exists() {
        let interface = artifact(
            "pub fun consume: (value: ()) -> Int32 = { 1 }",
            vec![ComponentExportSelection::new("api", "consume", "consume")],
        );
        let error = experimental_package_scalar_component("(module)", &interface).unwrap_err();
        assert!(matches!(
            error,
            ScalarComponentPackagingError::UnitParameterRequiresAdapter(source)
                if source == "consume"
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn componentizer_does_not_treat_wat_comment_as_a_core_export() {
        let interface = artifact(
            "pub fun answer: () -> Int32 = { 1 }",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        let core_wat = r#"
(module
  (func $answer (result i32) i32.const 1)
  ;; (export "answer" (func $answer))
)
"#;
        let error = experimental_package_scalar_component(core_wat, &interface).unwrap_err();
        assert!(matches!(
            error,
            ScalarComponentPackagingError::MissingCoreSourceExport(source) if source == "answer"
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn componentizer_rejects_imported_core_function() {
        let interface = artifact(
            "pub fun answer: () -> Int32 = { 1 }",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        let core_wat = r#"
(module
  (import "host" "answer" (func $answer (result i32)))
  (export "answer" (func $answer))
)
"#;
        let error = experimental_package_scalar_component(core_wat, &interface).unwrap_err();
        assert!(matches!(
            error,
            ScalarComponentPackagingError::CoreImportsUnsupported
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn componentizer_rejects_tampered_interface_artifact() {
        let mut interface = artifact(
            "pub fun answer: () -> Int32 = { 1 }",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        interface.normalized_wit = interface.normalized_wit.replace("s32", "s64");

        let error = experimental_package_scalar_component("(module)", &interface).unwrap_err();
        assert!(matches!(
            error,
            ScalarComponentPackagingError::InvalidInterfaceArtifact(_)
        ));
    }

    #[test]
    fn raw_root_catalog_does_not_publish_imported_public_functions() {
        let root = parse(
            r#"
import dependency.{foreign_value}

pub fun root_value: () -> Int32 = { 1 }
"#,
        );
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&root).unwrap();
        assert!(catalog.contains("root_value"));
        assert!(!catalog.contains("foreign_value"));

        let resolved = resolve_program_imports_with_module_source_map(
            root,
            HashMap::from([(
                "dependency".to_string(),
                "pub fun foreign_value: () -> Int32 = { 2 }".to_string(),
            )]),
        )
        .unwrap();
        let checked = checked(&resolved);
        let error = build_component_interface(
            &catalog,
            &resolved,
            &checked,
            &ComponentSelection::v1(
                "test:root@0.1.0",
                "root",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new(
                    "api",
                    "foreign_value",
                    "foreign-value",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(
            error,
            ComponentInterfaceError::NotRootPublicFunction("foreign_value".to_string())
        );
    }

    #[test]
    fn rich_contracts_lower_but_open_generic_functions_fail_closed() {
        let string_root = parse(
            r#"
pub fun echo: (value: String) -> String = { value }
"#,
        );
        let string_catalog = RootPublicFunctionCatalog::from_unresolved_root(&string_root).unwrap();
        let string_checked = checked(&string_root);
        let string_artifact = build_component_interface(
            &string_catalog,
            &string_root,
            &string_checked,
            &ComponentSelection::v1(
                "test:closed@0.1.0",
                "closed",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new("api", "echo", "echo")],
            ),
        )
        .unwrap();
        assert!(string_artifact
            .normalized_wit
            .contains("echo: func(value: string) -> string"));
        assert_eq!(
            string_artifact.ir.implementation_subset,
            COMPONENT_RICH_INTERFACE_SUBSET
        );

        let generic_root = parse(
            r#"
pub fun constant: <T>() -> Int32 = { 1 }
"#,
        );
        let generic_catalog =
            RootPublicFunctionCatalog::from_unresolved_root(&generic_root).unwrap();
        let generic_checked = checked(&generic_root);
        let generic_error = build_component_interface(
            &generic_catalog,
            &generic_root,
            &generic_checked,
            &ComponentSelection::v1(
                "test:closed@0.1.0",
                "closed",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new("api", "constant", "constant")],
            ),
        )
        .unwrap_err();
        assert_eq!(
            generic_error,
            ComponentInterfaceError::OpenGenericUnsupported("constant".to_string())
        );
    }

    #[test]
    fn checked_named_type_graph_lowers_rich_contracts_deterministically() {
        let artifact = artifact(
            r#"
pub record Envelope<T> {
    value: T,
    label: String,
    items: List<Int32>,
    fixed: Array<Boolean, 3>,
    maybe: Option<Char>,
    outcome: Result<Int64, String>
}

pub enum Status {
    Ready
    Failed
}

pub enum Event {
    Idle
    Message(String)
    Code(Int32)
}

pub fun keep_envelope: (value: Envelope<Int32>) -> Envelope<Int32> = { value }
pub fun keep_status: (value: Status) -> Status = { value }
pub fun keep_event: (value: Event) -> Event = { value }
"#,
            vec![
                ComponentExportSelection::new("api", "keep_envelope", "keep-envelope"),
                ComponentExportSelection::new("api", "keep_status", "keep-status"),
                ComponentExportSelection::new("api", "keep_event", "keep-event"),
            ],
        );

        assert_eq!(
            artifact.ir.implementation_subset,
            COMPONENT_RICH_INTERFACE_SUBSET
        );
        validate_generated_wit(&artifact.normalized_wit).unwrap();

        let interface = &artifact.ir.interfaces[0];
        let envelope = interface
            .types
            .iter()
            .find(|ty| ty.source_type == "Envelope<Int32>")
            .expect("concrete generic record should be emitted");
        assert!(envelope.name.starts_with("envelope-g-"));
        let ComponentNamedTypeKind::Record { fields } = &envelope.kind else {
            panic!("Envelope<Int32> should lower to a WIT record");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            vec!["value", "label", "items", "fixed", "maybe", "outcome"]
        );
        assert_eq!(fields[0].ty, ComponentValueType::S32);
        assert_eq!(fields[1].ty, ComponentValueType::String);
        assert_eq!(
            fields[2].ty,
            ComponentValueType::List(Box::new(ComponentValueType::S32))
        );
        assert_eq!(
            fields[3].ty,
            ComponentValueType::Array {
                element: Box::new(ComponentValueType::Bool),
                length: 3,
            }
        );
        assert_eq!(
            fields[4].ty,
            ComponentValueType::Option(Box::new(ComponentValueType::Char))
        );
        assert_eq!(
            fields[5].ty,
            ComponentValueType::Result {
                ok: Box::new(ComponentValueType::S64),
                err: Box::new(ComponentValueType::String),
            }
        );

        let status = interface
            .types
            .iter()
            .find(|ty| ty.source_type == "Status")
            .expect("payload-free enum should be emitted");
        assert!(matches!(status.kind, ComponentNamedTypeKind::Enum { .. }));
        let event = interface
            .types
            .iter()
            .find(|ty| ty.source_type == "Event")
            .expect("payload enum should be emitted");
        assert!(matches!(event.kind, ComponentNamedTypeKind::Variant { .. }));

        assert!(artifact
            .normalized_wit
            .contains(&format!("record {} {{", envelope.name)));
        assert!(artifact.normalized_wit.contains("fixed: list<bool>"));
        assert!(artifact
            .normalized_wit
            .contains("outcome: result<s64, string>"));
        assert!(artifact.normalized_wit.contains("enum status {"));
        assert!(artifact.normalized_wit.contains("variant event {"));
        assert!(artifact.normalized_wit.contains("message(string)"));

        let metadata_envelope = artifact.metadata.interfaces[0]
            .types
            .iter()
            .find(|ty| ty.name == envelope.name)
            .expect("stable record metadata should be emitted");
        let ComponentNamedTypeMetadataKind::Record { fields } = &metadata_envelope.kind else {
            panic!("stable Envelope metadata should be a record");
        };
        assert!(matches!(
            fields[3].ty,
            ComponentValueType::Array { length: 3, .. }
        ));
    }

    #[test]
    fn fixed_array_length_is_stable_metadata_even_when_wit_is_identical() {
        let length_two = artifact(
            "pub fun keep: (value: Array<Int32, 2>) -> Array<Int32, 2> = { value }",
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );
        let length_three = artifact(
            "pub fun keep: (value: Array<Int32, 3>) -> Array<Int32, 3> = { value }",
            vec![ComponentExportSelection::new("api", "keep", "keep")],
        );

        assert_eq!(length_two.normalized_wit, length_three.normalized_wit);
        assert_ne!(length_two.metadata, length_three.metadata);
        assert_ne!(length_two.interface_hash, length_three.interface_hash);
        assert!(length_two
            .normalized_wit
            .contains("keep: func(value: list<s32>) -> list<s32>"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn rich_adapter_generation_succeeds_while_legacy_scalar_packaging_fails() {
        let snapshot = tempfile::tempdir().expect("component snapshot should be creatable");
        let entry_path = snapshot.path().join("main.rl");
        let source = "pub fun echo: (value: String) -> String = { value }";
        let selection = ComponentSelection::v1(
            "test:rich-gate@0.1.0",
            "rich-gate",
            COMPONENT_ABI_PROFILE,
            vec![ComponentExportSelection::new("api", "echo", "echo")],
        );

        let compiled = compile_component(ComponentCompileRequest {
            source,
            entry_path: &entry_path,
            package_roots: &[],
            selection: &selection,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect("production compilation must include rich Canonical ABI adapters");
        assert_eq!(
            compiled.implementation_subset,
            COMPONENT_RICH_INTERFACE_SUBSET
        );
        assert!(!compiled.component_wasm.is_empty());

        let rich_interface = artifact(
            source,
            vec![ComponentExportSelection::new("api", "echo", "echo")],
        );
        let packaging_error =
            experimental_package_scalar_component("(module)", &rich_interface).unwrap_err();
        assert!(matches!(
            packaging_error,
            ScalarComponentPackagingError::RichBoundaryRequiresAdapter {
                ref interface,
                ref function,
            } if interface == "api" && function == "echo"
        ));
    }

    #[test]
    fn selection_order_does_not_change_wit_metadata_or_hash() {
        let source = r#"
pub fun first: (value: Int32) -> Int32 = { value }
pub fun second: (value: Int64) -> Int64 = { value }
"#;
        let first = artifact(
            source,
            vec![
                ComponentExportSelection::new("api", "first", "first"),
                ComponentExportSelection::new("admin", "second", "second"),
            ],
        );
        let second = artifact(
            source,
            vec![
                ComponentExportSelection::new("admin", "second", "second"),
                ComponentExportSelection::new("api", "first", "first"),
            ],
        );

        assert_eq!(first.ir, second.ir);
        assert_eq!(first.normalized_wit, second.normalized_wit);
        assert_eq!(first.metadata, second.metadata);
        assert_eq!(first.interface_hash, second.interface_hash);
    }

    #[test]
    fn semantic_hash_ignores_body_but_tracks_interface_changes() {
        let first = artifact(
            "pub fun answer: () -> Int32 = { 1 }",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        let body_changed = artifact(
            "pub fun answer: () -> Int32 = { 2 }",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        let interface_changed = artifact(
            "pub fun answer: () -> Int64 = { 2 }",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );

        assert_eq!(first.interface_hash, body_changed.interface_hash);
        assert_ne!(first.interface_hash, interface_changed.interface_hash);

        let changed_world_wit = first.normalized_wit.replace("  export api;\n", "");
        assert_ne!(
            first.interface_hash,
            component_interface_hash(&changed_world_wit, &first.metadata),
            "a meaningful normalized-WIT change must alter the interface hash"
        );
    }

    #[test]
    fn invalid_version_duplicate_selection_and_invalid_wit_fail_closed() {
        let root = parse("pub fun answer: () -> Int32 = { 1 }");
        let catalog = RootPublicFunctionCatalog::from_unresolved_root(&root).unwrap();
        let answer_checked = checked(&root);

        let mut unsupported = ComponentSelection::v1(
            "test:version@0.1.0",
            "version",
            COMPONENT_ABI_PROFILE,
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        unsupported.schema_version = 2;
        assert_eq!(
            build_component_interface(&catalog, &root, &answer_checked, &unsupported).unwrap_err(),
            ComponentInterfaceError::UnsupportedSelectionVersion(2)
        );

        let unsupported_abi = ComponentSelection::v1(
            "test:version@0.1.0",
            "version",
            "canonical-v2-async",
            vec![ComponentExportSelection::new("api", "answer", "answer")],
        );
        assert_eq!(
            build_component_interface(&catalog, &root, &answer_checked, &unsupported_abi)
                .unwrap_err(),
            ComponentInterfaceError::UnsupportedAbiProfile("canonical-v2-async".to_string())
        );

        let duplicate = build_component_interface(
            &catalog,
            &root,
            &answer_checked,
            &ComponentSelection::v1(
                "test:duplicate@0.1.0",
                "duplicate",
                COMPONENT_ABI_PROFILE,
                vec![
                    ComponentExportSelection::new("api", "answer", "first"),
                    ComponentExportSelection::new("admin", "answer", "second"),
                ],
            ),
        )
        .unwrap_err();
        assert_eq!(
            duplicate,
            ComponentInterfaceError::DuplicateSourceSelection("answer".to_string())
        );

        let duplicate_wit_name_root = parse(
            r#"
pub fun first: () -> Int32 = { 1 }
pub fun second: () -> Int32 = { 2 }
"#,
        );
        let duplicate_wit_name_catalog =
            RootPublicFunctionCatalog::from_unresolved_root(&duplicate_wit_name_root).unwrap();
        let duplicate_wit_name_checked = checked(&duplicate_wit_name_root);
        let duplicate_wit_name = build_component_interface(
            &duplicate_wit_name_catalog,
            &duplicate_wit_name_root,
            &duplicate_wit_name_checked,
            &ComponentSelection::v1(
                "test:duplicate@0.1.0",
                "duplicate",
                COMPONENT_ABI_PROFILE,
                vec![
                    ComponentExportSelection::new("api", "first", "same"),
                    ComponentExportSelection::new("api", "second", "same"),
                ],
            ),
        )
        .unwrap_err();
        assert_eq!(
            duplicate_wit_name,
            ComponentInterfaceError::DuplicateExternalName("api.same".to_string())
        );

        let invalid_wit = build_component_interface(
            &catalog,
            &root,
            &answer_checked,
            &ComponentSelection::v1(
                "not a package",
                "duplicate",
                COMPONENT_ABI_PROFILE,
                vec![ComponentExportSelection::new("api", "answer", "answer")],
            ),
        )
        .unwrap_err();
        assert!(matches!(
            invalid_wit,
            ComponentInterfaceError::GeneratedWitRejected(_)
        ));
    }
}
