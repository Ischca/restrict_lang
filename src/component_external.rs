//! External-WIT-first Component contracts, bindings, and compilation.
//!
//! This module deliberately keeps externally owned WIT authoritative. It
//! resolves a caller-supplied package, selects one explicit world, derives a
//! deterministic Restrict-facing recursive obligation IR and binding plan,
//! compiles the synchronous value subset against that plan, and validates that
//! supplied or generated Components target the selected world. Rich imports
//! are lowered and lifted through a compiler-owned Standard32 adapter while
//! resource/callable handles remain on their distinct planned adapter path.

#[path = "component_external_adapter.rs"]
mod external_adapter;

use crate::ast::{FunDecl, Program, TopDecl};
use crate::codegen::{
    CanonicalAbiScalarImport, CanonicalAbiScalarType, WasmCodeGen, WasmOptimizationLevel,
    WasmTargetProfile,
};
use crate::component::{
    ComponentEnumCaseIr, ComponentNamedTypeIr, ComponentNamedTypeKind, ComponentRecordFieldIr,
    ComponentValueType, ComponentVariantCaseIr,
};
use crate::ir::builder::{build_checked_ir, CheckedFunctionIr, CheckedProgramIr};
use crate::module::resolve_program_imports_for_file_with_package_roots;
use crate::parser::parse_program;
use crate::type_checker::{TypeChecker, TypedType};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;
use wit_parser::{
    Function, FunctionKind, InterfaceId, ManglingAndAbi, PackageId, Resolve, Type, TypeDefKind,
    TypeId, TypeOwner, WasmExport, WasmExportKind, WasmImport, WorldId, WorldItem, WorldKey,
};

/// Schema version for the external-WIT recursive binding IR.
pub const EXTERNAL_WIT_BINDING_IR_SCHEMA_VERSION: u32 = 2;

/// Schema version for generated Restrict binding names and Core import plans.
pub const EXTERNAL_WIT_RESTRICT_BINDING_PLAN_SCHEMA_VERSION: u32 = 2;

/// Schema version for stable adapter facts that WIT cannot express.
pub const EXTERNAL_WIT_ADAPTER_METADATA_SCHEMA_VERSION: u32 = 1;

/// ABI profile accepted by the initial external-WIT binding slice.
pub const EXTERNAL_WIT_SYNC_ABI_PROFILE: &str = "canonical-v1-sync";

/// Recursive synchronous value subset projected into Restrict bindings.
pub const EXTERNAL_WIT_SYNC_RICH_IMPLEMENTATION_SUBSET: &str = "external-wit-sync-rich-v1";

/// Legacy name retained for callers that identify the executable scalar seam.
pub const EXTERNAL_WIT_SYNC_SCALAR_IMPLEMENTATION_SUBSET: &str = "external-wit-sync-scalar-v1";

/// Authority recorded on every external-WIT binding IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentInterfaceAuthority {
    /// The WIT package is owned outside Restrict and remains the source of truth.
    ExternalWit,
}

/// Whether a function is consumed as an import or checked as an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExternalWitDirection {
    Import,
    Export,
}

impl ExternalWitDirection {
    fn label(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Export => "export",
        }
    }
}

/// Scalar values supported by the external-WIT binding foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalWitScalarType {
    S32,
    S64,
    F64,
    Bool,
    Char,
}

impl From<ExternalWitScalarType> for ComponentValueType {
    fn from(value: ExternalWitScalarType) -> Self {
        match value {
            ExternalWitScalarType::S32 => Self::S32,
            ExternalWitScalarType::S64 => Self::S64,
            ExternalWitScalarType::F64 => Self::F64,
            ExternalWitScalarType::Bool => Self::Bool,
            ExternalWitScalarType::Char => Self::Char,
        }
    }
}

/// Stable exact-length refinement for one otherwise ordinary WIT `list` site.
///
/// WIT alone cannot distinguish Restrict `List<T>` from `Array<T, N>`. The
/// caller therefore names the canonical list site explicitly. Unknown,
/// duplicate, or non-list sites are rejected rather than guessed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExternalWitArrayLengthMetadata {
    pub canonical_list_site: String,
    pub exact_length: u32,
}

impl ExternalWitArrayLengthMetadata {
    pub fn new(canonical_list_site: impl Into<String>, exact_length: u32) -> Self {
        Self {
            canonical_list_site: canonical_list_site.into(),
            exact_length,
        }
    }
}

/// Stable adapter facts supplementing an externally authoritative WIT world.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalWitAdapterMetadata {
    pub array_lengths: Vec<ExternalWitArrayLengthMetadata>,
}

/// One WIT list location discoverable from the recursive binding graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitListSite {
    pub canonical_path: String,
    /// `Some(N)` means this site is exposed to Restrict as `Array<T, N>`.
    pub exact_length: Option<u32>,
}

/// Rich synchronous features present in a binding or implementation boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExternalWitRichFeature {
    String,
    List,
    Array,
    Record,
    Option,
    Result,
    Enum,
    Variant,
}

/// Core WebAssembly value kinds in a Standard32 Canonical ABI signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalWitCanonicalCoreType {
    I32,
    I64,
    F32,
    F64,
}

/// Exact Canonical ABI Core signature generated by the pinned WIT resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitCanonicalCoreSignature {
    pub params: Vec<ExternalWitCanonicalCoreType>,
    pub results: Vec<ExternalWitCanonicalCoreType>,
}

/// One parameter in a Restrict binding or implementation obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitParam {
    pub name: String,
    pub ty: ComponentValueType,
}

/// One synchronous function derived from the selected external world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitFunctionBinding {
    /// Stable identity independent of WIT declaration traversal order.
    pub canonical_identity: String,
    pub direction: ExternalWitDirection,
    /// Fully qualified interface identity, or `None` for a world-level function.
    pub interface: Option<String>,
    pub name: String,
    pub params: Vec<ExternalWitParam>,
    /// `None` represents a WIT function with no result.
    pub result: Option<ComponentValueType>,
}

/// Deterministic Restrict-facing projection of one selected external WIT world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitBindingIr {
    pub schema_version: u32,
    pub authority: ComponentInterfaceAuthority,
    pub abi_profile: &'static str,
    pub implementation_subset: &'static str,
    /// Complete `namespace:name@version` package identity when versioned.
    pub package: String,
    pub world: String,
    /// Complete `namespace:name/world@version` selected-world identity.
    pub canonical_world_identity: String,
    /// Hash of the resolved external WIT package's semantic binary encoding.
    pub contract_hash: [u8; 32],
    /// Hash of normalized stable adapter metadata not expressible in WIT.
    pub adapter_metadata_hash: [u8; 32],
    pub adapter_metadata: ExternalWitAdapterMetadata,
    /// Reachable WIT nominal declarations in stable canonical identity order.
    pub types: Vec<ComponentNamedTypeIr>,
    /// Every list location, including whether it was refined to an array.
    pub list_sites: Vec<ExternalWitListSite>,
    /// Functions that generated Restrict consumer bindings must import.
    pub imports: Vec<ExternalWitFunctionBinding>,
    /// Functions that a Restrict implementation would be obliged to export.
    pub exports: Vec<ExternalWitFunctionBinding>,
}

/// One deterministic private Restrict function backed by a Standard32 import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitRestrictImportBinding {
    pub canonical_identity: String,
    pub interface: Option<String>,
    pub name: String,
    /// Generated Restrict identifier referenced by consumer source.
    pub local_function: String,
    /// Exact Standard32 Core module recognized by `wit-component`.
    pub core_module: String,
    /// Exact Standard32 Core field recognized by `wit-component`.
    pub core_name: String,
    pub params: Vec<ExternalWitParam>,
    pub result: Option<ComponentValueType>,
    pub canonical_signature: ExternalWitCanonicalCoreSignature,
}

/// One external-WIT export that must be mapped to an explicit root `pub fun`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitRestrictExportObligation {
    pub canonical_identity: String,
    pub interface: Option<String>,
    pub name: String,
    /// Exact Standard32 Core export alias consumed by `wit-component`.
    pub canonical_core_name: String,
    pub params: Vec<ExternalWitParam>,
    pub result: Option<ComponentValueType>,
    pub canonical_signature: ExternalWitCanonicalCoreSignature,
}

/// Deterministic language bindings and implementation obligations for one world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitRestrictBindingPlan {
    pub schema_version: u32,
    pub contract_hash: [u8; 32],
    pub adapter_metadata_hash: [u8; 32],
    pub adapter_metadata: ExternalWitAdapterMetadata,
    pub types: Vec<ComponentNamedTypeIr>,
    pub list_sites: Vec<ExternalWitListSite>,
    pub rich_features: Vec<ExternalWitRichFeature>,
    pub imports: Vec<ExternalWitRestrictImportBinding>,
    pub exports: Vec<ExternalWitRestrictExportObligation>,
    /// Private declarations injected before type checking. Their placeholder
    /// bodies never reach Core Wasm; codegen replaces them with the planned
    /// Standard32 imports.
    pub stub_source: String,
}

/// Explicit implementation mapping for one externally owned WIT export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitExportImplementation {
    pub canonical_identity: String,
    pub source_function: String,
}

impl ExternalWitExportImplementation {
    pub fn new(canonical_identity: impl Into<String>, source_function: impl Into<String>) -> Self {
        Self {
            canonical_identity: canonical_identity.into(),
            source_function: source_function.into(),
        }
    }
}

/// Immutable compiler request for an externally authoritative synchronous world.
pub struct ExternalWitCompileRequest<'a> {
    pub source: &'a str,
    pub entry_path: &'a Path,
    pub package_roots: &'a [(String, PathBuf)],
    pub contract: &'a ExternalWitContract,
    pub export_implementations: &'a [ExternalWitExportImplementation],
    pub optimization_level: WasmOptimizationLevel,
    pub arena_size_bytes: u32,
}

/// Validated Restrict consumer Component built from an external WIT contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalWitBuildArtifact {
    pub component_wasm: Vec<u8>,
    pub binding_plan: ExternalWitRestrictBindingPlan,
    pub contract_hash: [u8; 32],
    pub adapter_metadata_hash: [u8; 32],
    pub package: String,
    pub world: String,
    pub abi_profile: String,
    pub implementation_subset: String,
}

/// A resolved, externally authoritative WIT contract.
///
/// `Resolve` and `WorldId` stay paired privately so conformance cannot
/// accidentally validate against a different parse or a guessed world.
#[derive(Debug, Clone)]
pub struct ExternalWitContract {
    binding_ir: ExternalWitBindingIr,
    binding_plan: ExternalWitRestrictBindingPlan,
    resolve: Resolve,
    world: WorldId,
    encoded_package: Vec<u8>,
    adapter_wit: String,
}

impl ExternalWitContract {
    /// Parse WIT text and select an explicit world.
    pub fn from_text(logical_path: &str, wit: &str, world: &str) -> Result<Self, ExternalWitError> {
        Self::from_text_with_metadata(
            logical_path,
            wit,
            world,
            ExternalWitAdapterMetadata::default(),
        )
    }

    /// Parse WIT text with explicit stable adapter facts WIT cannot express.
    pub fn from_text_with_metadata(
        logical_path: &str,
        wit: &str,
        world: &str,
        metadata: ExternalWitAdapterMetadata,
    ) -> Result<Self, ExternalWitError> {
        require_explicit_world(world)?;
        let mut resolve = Resolve::default();
        let package = resolve.push_str(logical_path, wit).map_err(|error| {
            ExternalWitError::WitResolution {
                input: logical_path.to_string(),
                message: error.to_string(),
            }
        })?;
        Self::from_resolve(resolve, package, world, metadata)
    }

    /// Parse a WIT file, directory (including its `deps`), or encoded WIT package.
    pub fn from_path(path: &Path, world: &str) -> Result<Self, ExternalWitError> {
        Self::from_path_with_metadata(path, world, ExternalWitAdapterMetadata::default())
    }

    /// Parse a WIT path with explicit stable adapter facts WIT cannot express.
    pub fn from_path_with_metadata(
        path: &Path,
        world: &str,
        metadata: ExternalWitAdapterMetadata,
    ) -> Result<Self, ExternalWitError> {
        require_explicit_world(world)?;
        let mut resolve = Resolve::default();
        let (package, _) =
            resolve
                .push_path(path)
                .map_err(|error| ExternalWitError::WitResolution {
                    input: path.display().to_string(),
                    message: error.to_string(),
                })?;
        Self::from_resolve(resolve, package, world, metadata)
    }

    fn from_resolve(
        resolve: Resolve,
        package: PackageId,
        world_selector: &str,
        metadata: ExternalWitAdapterMetadata,
    ) -> Result<Self, ExternalWitError> {
        let world = resolve
            .select_world(&[package], Some(world_selector))
            .map_err(|error| ExternalWitError::WorldSelection {
                world: world_selector.to_string(),
                message: error.to_string(),
            })?;
        let encoded_package = wit_component::encode(&resolve, package).map_err(|error| {
            ExternalWitError::ContractEncoding {
                message: error.to_string(),
            }
        })?;
        let nested_packages = resolve
            .topological_packages()
            .into_iter()
            .filter(|candidate| *candidate != package)
            .collect::<Vec<_>>();
        let mut printer = wit_component::WitPrinter::default();
        printer.emit_docs(false);
        printer
            .print(&resolve, package, &nested_packages)
            .map_err(|error| ExternalWitError::ContractEncoding {
                message: format!("failed to normalize WIT for Canonical ABI generation: {error}"),
            })?;
        let adapter_wit = printer.output.to_string();
        let contract_hash = Sha256::digest(&encoded_package).into();
        let binding_ir = build_binding_ir(&resolve, world, contract_hash, metadata)?;
        let binding_plan = build_restrict_binding_plan(&resolve, world, &binding_ir)?;

        Ok(Self {
            binding_ir,
            binding_plan,
            resolve,
            world,
            encoded_package,
            adapter_wit,
        })
    }

    /// Deterministic Restrict binding and conformance obligations.
    pub fn binding_ir(&self) -> &ExternalWitBindingIr {
        &self.binding_ir
    }

    /// Deterministic generated Restrict stubs and Standard32 import/export plan.
    pub fn restrict_binding_plan(&self) -> &ExternalWitRestrictBindingPlan {
        &self.binding_plan
    }

    /// Semantic binary serialization of the authoritative external WIT package.
    pub fn encoded_wit_package(&self) -> &[u8] {
        &self.encoded_package
    }

    /// Validate Component encoding and conformance to the selected external world.
    ///
    /// This proves structural targeting only. It does not instantiate or execute
    /// the Component and therefore is not the Rust-produced-to-Restrict E2E gate.
    pub fn validate_component(
        &self,
        component_wasm: &[u8],
    ) -> Result<ComponentConformanceReport, ExternalWitError> {
        if !wasmparser::Parser::is_component(component_wasm) {
            return Err(ExternalWitError::ComponentRequired);
        }
        wasmparser::Validator::new_with_features(
            wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
        )
        .validate_all(component_wasm)
        .map_err(|error| ExternalWitError::ComponentValidation {
            message: error.to_string(),
        })?;
        wit_component::targets(&self.resolve, self.world, component_wasm).map_err(|error| {
            ExternalWitError::ComponentConformance {
                world: self.binding_ir.canonical_world_identity.clone(),
                message: error.to_string(),
            }
        })?;

        Ok(ComponentConformanceReport {
            canonical_world_identity: self.binding_ir.canonical_world_identity.clone(),
            contract_hash: self.binding_ir.contract_hash,
            adapter_metadata_hash: self.binding_ir.adapter_metadata_hash,
        })
    }
}

/// Evidence returned after Component validation and world conformance succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentConformanceReport {
    pub canonical_world_identity: String,
    pub contract_hash: [u8; 32],
    /// Stable non-WIT adapter facts checked by the Restrict boundary plan.
    pub adapter_metadata_hash: [u8; 32],
}

/// Typed failures from binding injection through validated Component output.
#[derive(Debug, Error)]
pub enum ExternalWitCompileError {
    #[error("Restrict source could not be parsed: {0}")]
    SourceParse(String),
    #[error("Restrict source contains unparsed input beginning with {0:?}")]
    SourceTrailingInput(String),
    #[error("generated external-WIT binding stubs could not be parsed: {0}")]
    GeneratedBindingParse(String),
    #[error("generated external-WIT binding stubs contain unparsed input beginning with {0:?}")]
    GeneratedBindingTrailingInput(String),
    #[error("generated external-WIT binding name '{0}' collides with root Restrict source")]
    GeneratedBindingCollision(String),
    #[error("root Restrict source declares public function '{0}' more than once")]
    DuplicateRootPublicFunction(String),
    #[error("Restrict module resolution failed: {0}")]
    ModuleResolution(String),
    #[error("Restrict type checking failed: {0}")]
    TypeCheck(String),
    #[error("checked Restrict IR construction failed: {0}")]
    CheckedIr(String),
    #[error("external-WIT export implementation names obligation '{0}' more than once")]
    DuplicateExportImplementation(String),
    #[error("external-WIT export implementation names unknown obligation '{0}'")]
    UnknownExportImplementation(String),
    #[error("external-WIT export implementations are missing obligations: {0}")]
    MissingExportImplementations(String),
    #[error("root Restrict function '{0}' is selected for more than one external-WIT export")]
    DuplicateSourceImplementation(String),
    #[error("external-WIT export maps to '{0}', which is not a root-source public function")]
    NotRootPublicFunction(String),
    #[error("external-WIT export maps to unsupported async, generic, or temporal function '{0}'")]
    UnsupportedSourceImplementation(String),
    #[error("checked Restrict IR has no external-WIT export implementation function '{0}'")]
    MissingCheckedFunction(String),
    #[error("checked Restrict function '{0}' is not marked source-public")]
    CheckedFunctionNotPublic(String),
    #[error("Restrict function '{source_function}' does not match external-WIT obligation '{canonical_identity}': {detail}")]
    ExportSignatureMismatch {
        canonical_identity: String,
        source_function: String,
        detail: String,
    },
    #[error("external-WIT Canonical ABI adapter generation failed: {0}")]
    CanonicalAdapter(String),
    #[error("Restrict Core Wasm generation failed: {0}")]
    CoreCodegen(String),
    #[error("Restrict Core WAT could not be encoded or validated: {0}")]
    CoreWat(String),
    #[error("Restrict Core Wasm is missing selected source function export '{0}'")]
    MissingCoreExport(String),
    #[error("Restrict Core export '{0}' is not a function")]
    CoreExportNotFunction(String),
    #[error("Restrict Core Wasm could not be re-encoded with Standard32 aliases: {0}")]
    CoreAlias(String),
    #[error("external-WIT Component metadata could not be embedded: {0}")]
    MetadataEmbedding(String),
    #[error("external-WIT Component encoding failed: {0}")]
    ComponentEncoding(String),
    #[error("external-WIT Component validation failed: {0}")]
    ComponentValidation(String),
    #[error("generated Component does not conform to external WIT world: {0}")]
    ComponentConformance(String),
}

impl ExternalWitCompileError {
    /// Stable pipeline stage for callers that render compiler diagnostics.
    pub fn stage(&self) -> &'static str {
        match self {
            Self::SourceParse(_)
            | Self::SourceTrailingInput(_)
            | Self::GeneratedBindingParse(_)
            | Self::GeneratedBindingTrailingInput(_)
            | Self::GeneratedBindingCollision(_)
            | Self::DuplicateRootPublicFunction(_) => "binding-injection",
            Self::ModuleResolution(_) => "module-resolution",
            Self::TypeCheck(_) => "type-check",
            Self::CheckedIr(_) => "checked-ir",
            Self::DuplicateExportImplementation(_)
            | Self::UnknownExportImplementation(_)
            | Self::MissingExportImplementations(_)
            | Self::DuplicateSourceImplementation(_)
            | Self::NotRootPublicFunction(_)
            | Self::UnsupportedSourceImplementation(_)
            | Self::MissingCheckedFunction(_)
            | Self::CheckedFunctionNotPublic(_)
            | Self::ExportSignatureMismatch { .. } => "implementation-conformance",
            Self::CanonicalAdapter(_)
            | Self::CoreCodegen(_)
            | Self::CoreWat(_)
            | Self::MissingCoreExport(_)
            | Self::CoreExportNotFunction(_)
            | Self::CoreAlias(_) => "core-lowering",
            Self::MetadataEmbedding(_) | Self::ComponentEncoding(_) => "component-packaging",
            Self::ComponentValidation(_) | Self::ComponentConformance(_) => "component-validation",
        }
    }
}

/// Typed diagnostics for the external-WIT-first foundation.
#[derive(Debug, Error)]
pub enum ExternalWitError {
    #[error("external WIT requires an explicit non-empty world selector")]
    MissingWorldSelection,
    #[error("external WIT parse/resolve failed for '{input}': {message}")]
    WitResolution { input: String, message: String },
    #[error("external WIT world selection '{world}' failed: {message}")]
    WorldSelection { world: String, message: String },
    #[error("external WIT selected world is not owned by a package")]
    WorldWithoutPackage,
    #[error("external WIT semantic package encoding failed: {message}")]
    ContractEncoding { message: String },
    #[error("external WIT async function is outside v0.1 at '{boundary}'")]
    UnsupportedAsync { boundary: String },
    #[error("external WIT resource requires the v0.1 resource adapter at '{boundary}'")]
    UnsupportedResource { boundary: String },
    #[error("external WIT {kind} is outside the v0.1 synchronous value mapping at '{boundary}'")]
    UnsupportedType {
        boundary: String,
        kind: &'static str,
    },
    #[error("external WIT type alias cycle at '{boundary}'")]
    TypeAliasCycle { boundary: String },
    #[error("external WIT adapter metadata names duplicate array site '{site}'")]
    DuplicateArrayMetadata { site: String },
    #[error("external WIT adapter metadata names unknown or non-list array site '{site}'")]
    UnknownArrayMetadata { site: String },
    #[error("external WIT named type at '{boundary}' has no stable owner or name")]
    InvalidNamedTypeIdentity { boundary: String },
    #[error("external WIT member names normalize to duplicate Restrict identifier '{name}' at '{boundary}'")]
    RestrictNameCollision { boundary: String, name: String },
    #[error("external WIT binding plan invariant failed: {message}")]
    BindingPlanInvariant { message: String },
    #[error("expected a WebAssembly Component, received Core Wasm or invalid bytes")]
    ComponentRequired,
    #[error("WebAssembly Component validation failed: {message}")]
    ComponentValidation { message: String },
    #[error("Component does not conform to external WIT world '{world}': {message}")]
    ComponentConformance { world: String, message: String },
}

impl ExternalWitError {
    /// Stable pipeline stage for machine-readable diagnostic routing.
    pub fn stage(&self) -> &'static str {
        match self {
            Self::MissingWorldSelection | Self::WorldSelection { .. } => "world-selection",
            Self::WitResolution { .. } => "wit-resolution",
            Self::WorldWithoutPackage | Self::ContractEncoding { .. } => "contract-identity",
            Self::UnsupportedAsync { .. }
            | Self::UnsupportedResource { .. }
            | Self::UnsupportedType { .. }
            | Self::TypeAliasCycle { .. }
            | Self::InvalidNamedTypeIdentity { .. }
            | Self::RestrictNameCollision { .. } => "binding-ir",
            Self::DuplicateArrayMetadata { .. } | Self::UnknownArrayMetadata { .. } => {
                "adapter-metadata"
            }
            Self::BindingPlanInvariant { .. } => "binding-plan",
            Self::ComponentRequired | Self::ComponentValidation { .. } => "component-validation",
            Self::ComponentConformance { .. } => "component-conformance",
        }
    }
}

/// Compile Restrict source as an implementation of one externally owned WIT
/// world. Generated import stubs participate in the same parse/resolve/check/IR
/// pipeline as user source, but the canonical adapter replaces their local
/// symbols with exact Standard32 Core imports before Component packaging.
pub fn compile_external_wit_component(
    request: ExternalWitCompileRequest<'_>,
) -> Result<ExternalWitBuildArtifact, ExternalWitCompileError> {
    let (remaining, mut unresolved_root) = parse_program(request.source)
        .map_err(|error| ExternalWitCompileError::SourceParse(format!("{error:?}")))?;
    if !remaining.trim().is_empty() {
        return Err(ExternalWitCompileError::SourceTrailingInput(
            remaining.chars().take(80).collect(),
        ));
    }

    let root_public = collect_root_public_functions(&unresolved_root)?;
    let root_declaration_names = collect_root_declaration_names(&unresolved_root);
    for binding in &request.contract.binding_plan.imports {
        if root_declaration_names.contains(&binding.local_function) {
            return Err(ExternalWitCompileError::GeneratedBindingCollision(
                binding.local_function.clone(),
            ));
        }
    }
    for ty in &request.contract.binding_plan.types {
        if root_declaration_names.contains(&ty.name) {
            return Err(ExternalWitCompileError::GeneratedBindingCollision(
                ty.name.clone(),
            ));
        }
    }

    let (stub_remaining, stubs) = parse_program(&request.contract.binding_plan.stub_source)
        .map_err(|error| ExternalWitCompileError::GeneratedBindingParse(format!("{error:?}")))?;
    if !stub_remaining.trim().is_empty() {
        return Err(ExternalWitCompileError::GeneratedBindingTrailingInput(
            stub_remaining.chars().take(80).collect(),
        ));
    }
    if !stubs.imports.is_empty() {
        return Err(ExternalWitCompileError::GeneratedBindingParse(
            "generated binding source unexpectedly contains module imports".to_string(),
        ));
    }
    unresolved_root.declarations.extend(stubs.declarations);

    let resolved_program = resolve_program_imports_for_file_with_package_roots(
        unresolved_root,
        request.entry_path,
        request.package_roots,
    )
    .map_err(|error| ExternalWitCompileError::ModuleResolution(error.to_string()))?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&resolved_program)
        .map_err(|error| ExternalWitCompileError::TypeCheck(error.to_string()))?;
    let checked_program = build_checked_ir(&resolved_program, &checker)
        .map_err(|error| ExternalWitCompileError::CheckedIr(error.to_string()))?;

    let resolved_exports = validate_export_implementations(
        request.contract.restrict_binding_plan(),
        request.export_implementations,
        &root_public,
        &checked_program,
    )?;
    let selected_source_functions = resolved_exports
        .iter()
        .map(|resolved| resolved.source_function.clone())
        .collect::<Vec<_>>();
    let has_rich_boundary = !request.contract.binding_plan.rich_features.is_empty();
    let core_imports = if has_rich_boundary {
        Vec::new()
    } else {
        request
            .contract
            .binding_plan
            .imports
            .iter()
            .map(|binding| CanonicalAbiScalarImport {
                source_function: binding.local_function.clone(),
                module: binding.core_module.clone(),
                name: binding.core_name.clone(),
                params: binding
                    .params
                    .iter()
                    .map(|param| {
                        canonical_core_scalar_type(&param.ty)
                            .expect("scalar boundary must have scalar Core parameters")
                    })
                    .collect(),
                result: binding.result.as_ref().map(|result| {
                    canonical_core_scalar_type(result)
                        .expect("scalar boundary must have a scalar Core result")
                }),
            })
            .collect::<Vec<_>>()
    };

    let mut codegen = WasmCodeGen::new()
        .with_target_profile(WasmTargetProfile::WasmCore)
        .with_optimization_level(request.optimization_level)
        .with_component_internal_exports(selected_source_functions)
        .with_canonical_abi_scalar_imports(core_imports)
        .with_arena_size_bytes(request.arena_size_bytes)
        .map_err(|error| ExternalWitCompileError::CoreCodegen(error.to_string()))?;
    let core_wat = codegen
        .generate_checked(&resolved_program, &checked_program)
        .map_err(|error| ExternalWitCompileError::CoreCodegen(error.to_string()))?;
    let (core_wat, packaged_exports) = if has_rich_boundary {
        let invocation_arena = codegen.component_adapter_arena_addr().ok_or_else(|| {
            ExternalWitCompileError::CanonicalAdapter(
                "Core generation did not reserve an external-WIT invocation arena".to_string(),
            )
        })?;
        let adapted = external_adapter::generate_external_import_adapter_core_wat(
            &core_wat,
            request.contract,
            &resolved_exports,
            invocation_arena,
        )
        .map_err(|error| ExternalWitCompileError::CanonicalAdapter(error.to_string()))?;
        let packaged_exports = resolved_exports
            .iter()
            .map(|implementation| ResolvedExportImplementation {
                source_function: adapted
                    .export_symbols
                    .get(&implementation.source_function)
                    .cloned()
                    .unwrap_or_else(|| implementation.source_function.clone()),
                canonical_core_name: implementation.canonical_core_name.clone(),
            })
            .collect::<Vec<_>>();
        (adapted.wat, packaged_exports)
    } else {
        (core_wat, resolved_exports)
    };
    let component_wasm =
        package_external_wit_component(&core_wat, request.contract, &packaged_exports)?;

    let ir = request.contract.binding_ir();
    Ok(ExternalWitBuildArtifact {
        component_wasm,
        binding_plan: request.contract.binding_plan.clone(),
        contract_hash: ir.contract_hash,
        adapter_metadata_hash: ir.adapter_metadata_hash,
        package: ir.package.clone(),
        world: ir.world.clone(),
        abi_profile: ir.abi_profile.to_string(),
        implementation_subset: ir.implementation_subset.to_string(),
    })
}

#[derive(Debug, Clone)]
struct ResolvedExportImplementation {
    source_function: String,
    canonical_core_name: String,
}

fn collect_root_public_functions(
    program: &Program,
) -> Result<BTreeMap<String, FunDecl>, ExternalWitCompileError> {
    let mut functions = BTreeMap::new();
    for declaration in &program.declarations {
        let TopDecl::Export(export) = declaration else {
            continue;
        };
        let TopDecl::Function(function) = export.item.as_ref() else {
            continue;
        };
        if functions
            .insert(function.name.clone(), function.clone())
            .is_some()
        {
            return Err(ExternalWitCompileError::DuplicateRootPublicFunction(
                function.name.clone(),
            ));
        }
    }
    Ok(functions)
}

fn collect_root_declaration_names(program: &Program) -> BTreeSet<String> {
    program
        .declarations
        .iter()
        .filter_map(root_declaration_name)
        .map(str::to_string)
        .collect()
}

fn root_declaration_name(declaration: &TopDecl) -> Option<&str> {
    match declaration {
        TopDecl::Function(function) => Some(function.name.as_str()),
        TopDecl::Record(record) => Some(record.name.as_str()),
        TopDecl::Enum(enum_) => Some(enum_.name.as_str()),
        TopDecl::Form(form) => Some(form.name.as_str()),
        TopDecl::Context(context) => Some(context.name.as_str()),
        TopDecl::Export(export) => root_declaration_name(export.item.as_ref()),
        _ => None,
    }
}

fn validate_export_implementations(
    plan: &ExternalWitRestrictBindingPlan,
    implementations: &[ExternalWitExportImplementation],
    root_public: &BTreeMap<String, FunDecl>,
    checked_program: &CheckedProgramIr,
) -> Result<Vec<ResolvedExportImplementation>, ExternalWitCompileError> {
    let obligations = plan
        .exports
        .iter()
        .map(|obligation| (obligation.canonical_identity.as_str(), obligation))
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeMap::<&str, &str>::new();
    let mut selected_sources = BTreeSet::new();
    for implementation in implementations {
        let identity = implementation.canonical_identity.as_str();
        if !obligations.contains_key(identity) {
            return Err(ExternalWitCompileError::UnknownExportImplementation(
                implementation.canonical_identity.clone(),
            ));
        }
        if selected
            .insert(identity, implementation.source_function.as_str())
            .is_some()
        {
            return Err(ExternalWitCompileError::DuplicateExportImplementation(
                implementation.canonical_identity.clone(),
            ));
        }
        if !selected_sources.insert(implementation.source_function.as_str()) {
            return Err(ExternalWitCompileError::DuplicateSourceImplementation(
                implementation.source_function.clone(),
            ));
        }
    }

    let missing = obligations
        .keys()
        .filter(|identity| !selected.contains_key(**identity))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ExternalWitCompileError::MissingExportImplementations(
            missing.join(", "),
        ));
    }

    let checked_by_name = checked_program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut resolved = Vec::with_capacity(plan.exports.len());
    for obligation in &plan.exports {
        let source_function = selected[obligation.canonical_identity.as_str()];
        let source = root_public.get(source_function).ok_or_else(|| {
            ExternalWitCompileError::NotRootPublicFunction(source_function.to_string())
        })?;
        if source.is_async
            || !source.type_params.is_empty()
            || !source.temporal_constraints.is_empty()
        {
            return Err(ExternalWitCompileError::UnsupportedSourceImplementation(
                source_function.to_string(),
            ));
        }
        let checked = checked_by_name.get(source_function).ok_or_else(|| {
            ExternalWitCompileError::MissingCheckedFunction(source_function.to_string())
        })?;
        if !checked.lowering.source_exported {
            return Err(ExternalWitCompileError::CheckedFunctionNotPublic(
                source_function.to_string(),
            ));
        }
        validate_export_signature(obligation, source_function, checked)?;
        resolved.push(ResolvedExportImplementation {
            source_function: source_function.to_string(),
            canonical_core_name: obligation.canonical_core_name.clone(),
        });
    }
    Ok(resolved)
}

fn validate_export_signature(
    obligation: &ExternalWitRestrictExportObligation,
    source_function: &str,
    checked: &CheckedFunctionIr,
) -> Result<(), ExternalWitCompileError> {
    if checked.params.len() != obligation.params.len() {
        return Err(export_signature_mismatch(
            obligation,
            source_function,
            format!(
                "expected {} parameters, found {}",
                obligation.params.len(),
                checked.params.len()
            ),
        ));
    }
    for (index, (actual, expected)) in checked.params.iter().zip(&obligation.params).enumerate() {
        if !component_value_matches_typed(&expected.ty, actual.final_type.as_typed_type()) {
            let mut expected_name = String::new();
            write_restrict_value_type(&mut expected_name, &expected.ty);
            return Err(export_signature_mismatch(
                obligation,
                source_function,
                format!("parameter {index} must be {expected_name}"),
            ));
        }
    }
    let result_matches = obligation.result.as_ref().map_or_else(
        || checked.return_type.as_typed_type() == &TypedType::Unit,
        |expected| component_value_matches_typed(expected, checked.return_type.as_typed_type()),
    );
    if !result_matches {
        let mut expected_name = String::new();
        if let Some(result) = &obligation.result {
            write_restrict_value_type(&mut expected_name, result);
        } else {
            expected_name.push_str("()");
        }
        return Err(export_signature_mismatch(
            obligation,
            source_function,
            format!("result must be {expected_name}"),
        ));
    }
    if !checked.monomorphic {
        return Err(export_signature_mismatch(
            obligation,
            source_function,
            "implementation must be monomorphic".to_string(),
        ));
    }
    Ok(())
}

fn export_signature_mismatch(
    obligation: &ExternalWitRestrictExportObligation,
    source_function: &str,
    detail: String,
) -> ExternalWitCompileError {
    ExternalWitCompileError::ExportSignatureMismatch {
        canonical_identity: obligation.canonical_identity.clone(),
        source_function: source_function.to_string(),
        detail,
    }
}

fn component_value_matches_typed(expected: &ComponentValueType, actual: &TypedType) -> bool {
    match (expected, actual) {
        (ComponentValueType::Unit, TypedType::Unit)
        | (ComponentValueType::S32, TypedType::Int32)
        | (ComponentValueType::S64, TypedType::Int64)
        | (ComponentValueType::F64, TypedType::Float64)
        | (ComponentValueType::Bool, TypedType::Boolean)
        | (ComponentValueType::Char, TypedType::Char)
        | (ComponentValueType::String, TypedType::String) => true,
        (ComponentValueType::List(expected), TypedType::List(actual))
        | (ComponentValueType::Option(expected), TypedType::Option(actual)) => {
            component_value_matches_typed(expected, actual)
        }
        (
            ComponentValueType::Array {
                element: expected,
                length,
            },
            TypedType::Array(actual, crate::type_checker::ArrayLength::Known(actual_length)),
        ) => length == actual_length && component_value_matches_typed(expected, actual),
        (
            ComponentValueType::Result {
                ok: expected_ok,
                err: expected_err,
            },
            TypedType::Result(actual_ok, actual_err),
        ) => {
            component_value_matches_typed(expected_ok, actual_ok)
                && component_value_matches_typed(expected_err, actual_err)
        }
        (ComponentValueType::Named(expected), TypedType::Record { name, .. })
        | (ComponentValueType::Named(expected), TypedType::Enum { name }) => expected == name,
        (ComponentValueType::OwnResource(_), _) | (ComponentValueType::OwnCallable(_), _) => false,
        _ => false,
    }
}

fn canonical_core_scalar_type(ty: &ComponentValueType) -> Option<CanonicalAbiScalarType> {
    match ty {
        ComponentValueType::S32 | ComponentValueType::Bool | ComponentValueType::Char => {
            Some(CanonicalAbiScalarType::I32)
        }
        ComponentValueType::S64 => Some(CanonicalAbiScalarType::I64),
        ComponentValueType::F64 => Some(CanonicalAbiScalarType::F64),
        ComponentValueType::Unit
        | ComponentValueType::String
        | ComponentValueType::List(_)
        | ComponentValueType::Array { .. }
        | ComponentValueType::Option(_)
        | ComponentValueType::Result { .. }
        | ComponentValueType::Named(_)
        | ComponentValueType::OwnResource(_)
        | ComponentValueType::OwnCallable(_) => None,
    }
}

fn package_external_wit_component(
    core_wat: &str,
    contract: &ExternalWitContract,
    exports: &[ResolvedExportImplementation],
) -> Result<Vec<u8>, ExternalWitCompileError> {
    use wit_component::{ComponentEncoder, StringEncoding};

    let core_wasm = wat::parse_str(core_wat)
        .map_err(|error| ExternalWitCompileError::CoreWat(error.to_string()))?;
    wasmparser::Validator::new()
        .validate_all(&core_wasm)
        .map_err(|error| ExternalWitCompileError::CoreWat(error.to_string()))?;

    let core_exports = collect_core_exports(&core_wasm)?;
    let aliases = exports
        .iter()
        .map(|implementation| {
            let (kind, index) = core_exports
                .get(&implementation.source_function)
                .ok_or_else(|| {
                    ExternalWitCompileError::MissingCoreExport(
                        implementation.source_function.clone(),
                    )
                })?;
            if *kind != wasmparser::ExternalKind::Func {
                return Err(ExternalWitCompileError::CoreExportNotFunction(
                    implementation.source_function.clone(),
                ));
            }
            Ok((*index, implementation.canonical_core_name.clone()))
        })
        .collect::<Result<Vec<_>, ExternalWitCompileError>>()?;
    let aliased_core_wasm = add_standard32_export_aliases(&core_wasm, &aliases)?;
    wasmparser::Validator::new()
        .validate_all(&aliased_core_wasm)
        .map_err(|error| ExternalWitCompileError::CoreWat(error.to_string()))?;

    let mut embedded_core_wasm = aliased_core_wasm;
    wit_component::embed_component_metadata(
        &mut embedded_core_wasm,
        &contract.resolve,
        contract.world,
        StringEncoding::UTF8,
    )
    .map_err(|error| ExternalWitCompileError::MetadataEmbedding(error.to_string()))?;
    let component_wasm = ComponentEncoder::default()
        .module(&embedded_core_wasm)
        .map_err(|error| ExternalWitCompileError::ComponentEncoding(error.to_string()))?
        .reject_legacy_names(true)
        .validate(true)
        .encode()
        .map_err(|error| ExternalWitCompileError::ComponentEncoding(error.to_string()))?;
    wasmparser::Validator::new_with_features(
        wasmparser::WasmFeatures::default() | wasmparser::WasmFeatures::COMPONENT_MODEL,
    )
    .validate_all(&component_wasm)
    .map_err(|error| ExternalWitCompileError::ComponentValidation(error.to_string()))?;
    wit_component::targets(&contract.resolve, contract.world, &component_wasm)
        .map_err(|error| ExternalWitCompileError::ComponentConformance(error.to_string()))?;
    Ok(component_wasm)
}

fn collect_core_exports(
    core_wasm: &[u8],
) -> Result<BTreeMap<String, (wasmparser::ExternalKind, u32)>, ExternalWitCompileError> {
    use wasmparser::{Encoding, Parser, Payload};

    let mut exports = BTreeMap::new();
    for payload in Parser::new(0).parse_all(core_wasm) {
        match payload.map_err(|error| ExternalWitCompileError::CoreWat(error.to_string()))? {
            Payload::Version { encoding, .. } if encoding != Encoding::Module => {
                return Err(ExternalWitCompileError::CoreWat(
                    "expected a Core Wasm module".to_string(),
                ));
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export
                        .map_err(|error| ExternalWitCompileError::CoreWat(error.to_string()))?;
                    exports.insert(export.name.to_string(), (export.kind, export.index));
                }
            }
            _ => {}
        }
    }
    Ok(exports)
}

fn add_standard32_export_aliases(
    core_wasm: &[u8],
    aliases: &[(u32, String)],
) -> Result<Vec<u8>, ExternalWitCompileError> {
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
        .map_err(|error| ExternalWitCompileError::CoreAlias(error.to_string()))?;
    Ok(module.finish())
}

fn require_explicit_world(world: &str) -> Result<(), ExternalWitError> {
    if world.trim().is_empty() {
        Err(ExternalWitError::MissingWorldSelection)
    } else {
        Ok(())
    }
}

fn build_binding_ir(
    resolve: &Resolve,
    world_id: WorldId,
    contract_hash: [u8; 32],
    metadata: ExternalWitAdapterMetadata,
) -> Result<ExternalWitBindingIr, ExternalWitError> {
    let world = &resolve.worlds[world_id];
    let package_id = world.package.ok_or(ExternalWitError::WorldWithoutPackage)?;
    let package = resolve.packages[package_id].name.to_string();
    let canonical_world_identity = resolve.id_of_name(package_id, &world.name);

    let normalized_metadata = normalize_adapter_metadata(metadata)?;
    let adapter_metadata = normalized_metadata.metadata;
    let array_lengths = normalized_metadata.array_lengths;
    let adapter_metadata_hash = normalized_metadata.hash;
    let mut lowerer = ExternalWitTypeLowerer::new(resolve, array_lengths);

    let imports = collect_world_functions(
        resolve,
        &canonical_world_identity,
        ExternalWitDirection::Import,
        &world.imports,
        &mut lowerer,
    )?;
    let exports = collect_world_functions(
        resolve,
        &canonical_world_identity,
        ExternalWitDirection::Export,
        &world.exports,
        &mut lowerer,
    )?;
    let (types, list_sites) = lowerer.finish()?;

    Ok(ExternalWitBindingIr {
        schema_version: EXTERNAL_WIT_BINDING_IR_SCHEMA_VERSION,
        authority: ComponentInterfaceAuthority::ExternalWit,
        abi_profile: EXTERNAL_WIT_SYNC_ABI_PROFILE,
        implementation_subset: EXTERNAL_WIT_SYNC_RICH_IMPLEMENTATION_SUBSET,
        package,
        world: world.name.clone(),
        canonical_world_identity,
        contract_hash,
        adapter_metadata_hash,
        adapter_metadata,
        types,
        list_sites,
        imports,
        exports,
    })
}

struct NormalizedExternalWitAdapterMetadata {
    metadata: ExternalWitAdapterMetadata,
    array_lengths: BTreeMap<String, u32>,
    hash: [u8; 32],
}

fn normalize_adapter_metadata(
    mut metadata: ExternalWitAdapterMetadata,
) -> Result<NormalizedExternalWitAdapterMetadata, ExternalWitError> {
    metadata.array_lengths.sort();
    let mut array_lengths = BTreeMap::new();
    for entry in &metadata.array_lengths {
        if array_lengths
            .insert(entry.canonical_list_site.clone(), entry.exact_length)
            .is_some()
        {
            return Err(ExternalWitError::DuplicateArrayMetadata {
                site: entry.canonical_list_site.clone(),
            });
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(EXTERNAL_WIT_ADAPTER_METADATA_SCHEMA_VERSION.to_le_bytes());
    for entry in &metadata.array_lengths {
        let site = entry.canonical_list_site.as_bytes();
        hasher.update((site.len() as u64).to_le_bytes());
        hasher.update(site);
        hasher.update(entry.exact_length.to_le_bytes());
    }
    Ok(NormalizedExternalWitAdapterMetadata {
        metadata,
        array_lengths,
        hash: hasher.finalize().into(),
    })
}

#[derive(Debug, Clone)]
struct Standard32FunctionName {
    module: Option<String>,
    name: String,
    signature: ExternalWitCanonicalCoreSignature,
}

fn build_restrict_binding_plan(
    resolve: &Resolve,
    world_id: WorldId,
    binding_ir: &ExternalWitBindingIr,
) -> Result<ExternalWitRestrictBindingPlan, ExternalWitError> {
    let world = &resolve.worlds[world_id];
    let import_names = collect_standard32_function_names(
        resolve,
        &binding_ir.canonical_world_identity,
        ExternalWitDirection::Import,
        &world.imports,
    )?;
    let export_names = collect_standard32_function_names(
        resolve,
        &binding_ir.canonical_world_identity,
        ExternalWitDirection::Export,
        &world.exports,
    )?;

    let mut imports = Vec::with_capacity(binding_ir.imports.len());
    let mut local_functions = BTreeSet::new();
    for binding in &binding_ir.imports {
        let names = import_names
            .get(&binding.canonical_identity)
            .ok_or_else(|| ExternalWitError::BindingPlanInvariant {
                message: format!(
                    "missing Standard32 import name for '{}'",
                    binding.canonical_identity
                ),
            })?;
        let module =
            names
                .module
                .clone()
                .ok_or_else(|| ExternalWitError::BindingPlanInvariant {
                    message: format!(
                        "Standard32 import '{}' has no Core module",
                        binding.canonical_identity
                    ),
                })?;
        let local_function = stable_restrict_import_name(binding);
        if !local_functions.insert(local_function.clone()) {
            return Err(ExternalWitError::BindingPlanInvariant {
                message: format!(
                    "generated Restrict import name collision at '{}'",
                    local_function
                ),
            });
        }
        imports.push(ExternalWitRestrictImportBinding {
            canonical_identity: binding.canonical_identity.clone(),
            interface: binding.interface.clone(),
            name: binding.name.clone(),
            local_function,
            core_module: module,
            core_name: names.name.clone(),
            params: binding.params.clone(),
            result: binding.result.clone(),
            canonical_signature: names.signature.clone(),
        });
    }

    let mut exports = Vec::with_capacity(binding_ir.exports.len());
    for binding in &binding_ir.exports {
        let names = export_names
            .get(&binding.canonical_identity)
            .ok_or_else(|| ExternalWitError::BindingPlanInvariant {
                message: format!(
                    "missing Standard32 export name for '{}'",
                    binding.canonical_identity
                ),
            })?;
        if names.module.is_some() {
            return Err(ExternalWitError::BindingPlanInvariant {
                message: format!(
                    "Standard32 export '{}' unexpectedly has a Core module",
                    binding.canonical_identity
                ),
            });
        }
        exports.push(ExternalWitRestrictExportObligation {
            canonical_identity: binding.canonical_identity.clone(),
            interface: binding.interface.clone(),
            name: binding.name.clone(),
            canonical_core_name: names.name.clone(),
            params: binding.params.clone(),
            result: binding.result.clone(),
            canonical_signature: names.signature.clone(),
        });
    }

    let rich_features = collect_binding_rich_features(binding_ir);
    let stub_source = emit_restrict_import_stubs(&binding_ir.types, &imports)?;
    Ok(ExternalWitRestrictBindingPlan {
        schema_version: EXTERNAL_WIT_RESTRICT_BINDING_PLAN_SCHEMA_VERSION,
        contract_hash: binding_ir.contract_hash,
        adapter_metadata_hash: binding_ir.adapter_metadata_hash,
        adapter_metadata: binding_ir.adapter_metadata.clone(),
        types: binding_ir.types.clone(),
        list_sites: binding_ir.list_sites.clone(),
        rich_features,
        imports,
        exports,
        stub_source,
    })
}

fn collect_standard32_function_names<'a>(
    resolve: &Resolve,
    world_identity: &str,
    direction: ExternalWitDirection,
    items: impl IntoIterator<Item = (&'a WorldKey, &'a WorldItem)>,
) -> Result<BTreeMap<String, Standard32FunctionName>, ExternalWitError> {
    let mut names = BTreeMap::new();
    for (key, item) in items {
        match item {
            WorldItem::Function(function) => {
                insert_standard32_function_name(
                    resolve,
                    world_identity,
                    direction,
                    None,
                    function,
                    &mut names,
                )?;
            }
            WorldItem::Interface { id, .. } => {
                let interface_identity = stable_interface_identity(resolve, world_identity, key);
                for function in resolve.interfaces[*id].functions.values() {
                    insert_standard32_function_name(
                        resolve,
                        world_identity,
                        direction,
                        Some((key, interface_identity.as_str())),
                        function,
                        &mut names,
                    )?;
                }
            }
            WorldItem::Type { .. } => {}
        }
    }
    Ok(names)
}

fn insert_standard32_function_name(
    resolve: &Resolve,
    world_identity: &str,
    direction: ExternalWitDirection,
    interface: Option<(&WorldKey, &str)>,
    function: &Function,
    names: &mut BTreeMap<String, Standard32FunctionName>,
) -> Result<(), ExternalWitError> {
    let canonical_identity = canonical_function_identity(
        world_identity,
        direction,
        interface.map(|(_, identity)| identity),
        function,
    );
    let signature = canonical_core_signature(resolve, direction, function);
    let core = match direction {
        ExternalWitDirection::Import => {
            let (module, name) = resolve.wasm_import_name(
                ManglingAndAbi::Standard32,
                WasmImport::Func {
                    interface: interface.map(|(key, _)| key),
                    func: function,
                },
            );
            Standard32FunctionName {
                module: Some(module),
                name,
                signature,
            }
        }
        ExternalWitDirection::Export => Standard32FunctionName {
            module: None,
            name: resolve.wasm_export_name(
                ManglingAndAbi::Standard32,
                WasmExport::Func {
                    interface: interface.map(|(key, _)| key),
                    func: function,
                    kind: WasmExportKind::Normal,
                },
            ),
            signature,
        },
    };
    if names.insert(canonical_identity.clone(), core).is_some() {
        return Err(ExternalWitError::BindingPlanInvariant {
            message: format!(
                "duplicate Standard32 function identity '{}'",
                canonical_identity
            ),
        });
    }
    Ok(())
}

fn canonical_core_signature(
    resolve: &Resolve,
    direction: ExternalWitDirection,
    function: &Function,
) -> ExternalWitCanonicalCoreSignature {
    use wit_parser::abi::AbiVariant;

    let variant = match direction {
        ExternalWitDirection::Import => AbiVariant::GuestImport,
        ExternalWitDirection::Export => AbiVariant::GuestExport,
    };
    let signature = resolve.wasm_signature(variant, function);
    ExternalWitCanonicalCoreSignature {
        params: signature
            .params
            .iter()
            .copied()
            .map(canonical_core_type)
            .collect(),
        results: signature
            .results
            .iter()
            .copied()
            .map(canonical_core_type)
            .collect(),
    }
}

fn canonical_core_type(ty: wit_parser::abi::WasmType) -> ExternalWitCanonicalCoreType {
    use wit_parser::abi::WasmType;

    match ty {
        WasmType::I32 | WasmType::Pointer | WasmType::Length => ExternalWitCanonicalCoreType::I32,
        WasmType::I64 | WasmType::PointerOrI64 => ExternalWitCanonicalCoreType::I64,
        WasmType::F32 => ExternalWitCanonicalCoreType::F32,
        WasmType::F64 => ExternalWitCanonicalCoreType::F64,
    }
}

fn collect_binding_rich_features(binding_ir: &ExternalWitBindingIr) -> Vec<ExternalWitRichFeature> {
    let mut features = BTreeSet::new();
    for binding in binding_ir.imports.iter().chain(&binding_ir.exports) {
        for param in &binding.params {
            collect_value_rich_features(&param.ty, &mut features);
        }
        if let Some(result) = &binding.result {
            collect_value_rich_features(result, &mut features);
        }
    }
    for declaration in &binding_ir.types {
        match &declaration.kind {
            ComponentNamedTypeKind::Record { fields } => {
                features.insert(ExternalWitRichFeature::Record);
                for field in fields {
                    collect_value_rich_features(&field.ty, &mut features);
                }
            }
            ComponentNamedTypeKind::Enum { .. } => {
                features.insert(ExternalWitRichFeature::Enum);
            }
            ComponentNamedTypeKind::Variant { cases } => {
                features.insert(ExternalWitRichFeature::Variant);
                for case in cases {
                    if let Some(payload) = &case.payload {
                        collect_value_rich_features(payload, &mut features);
                    }
                }
            }
        }
    }
    features.into_iter().collect()
}

fn collect_value_rich_features(
    ty: &ComponentValueType,
    features: &mut BTreeSet<ExternalWitRichFeature>,
) {
    match ty {
        ComponentValueType::String => {
            features.insert(ExternalWitRichFeature::String);
        }
        ComponentValueType::List(element) => {
            features.insert(ExternalWitRichFeature::List);
            collect_value_rich_features(element, features);
        }
        ComponentValueType::Array { element, .. } => {
            features.insert(ExternalWitRichFeature::Array);
            collect_value_rich_features(element, features);
        }
        ComponentValueType::Option(inner) => {
            features.insert(ExternalWitRichFeature::Option);
            collect_value_rich_features(inner, features);
        }
        ComponentValueType::Result { ok, err } => {
            features.insert(ExternalWitRichFeature::Result);
            collect_value_rich_features(ok, features);
            collect_value_rich_features(err, features);
        }
        ComponentValueType::Unit
        | ComponentValueType::S32
        | ComponentValueType::S64
        | ComponentValueType::F64
        | ComponentValueType::Bool
        | ComponentValueType::Char
        | ComponentValueType::Named(_)
        | ComponentValueType::OwnResource(_)
        | ComponentValueType::OwnCallable(_) => {}
    }
}

fn emit_restrict_import_stubs(
    types: &[ComponentNamedTypeIr],
    imports: &[ExternalWitRestrictImportBinding],
) -> Result<String, ExternalWitError> {
    let mut output = String::new();
    for ty in types {
        match &ty.kind {
            ComponentNamedTypeKind::Record { fields } => {
                output.push_str("record ");
                output.push_str(&ty.name);
                output.push_str(" {\n");
                for field in fields {
                    output.push_str("    ");
                    output.push_str(&field.name);
                    output.push_str(": ");
                    write_restrict_value_type(&mut output, &field.ty);
                    output.push('\n');
                }
                output.push_str("}\n\n");
            }
            ComponentNamedTypeKind::Enum { cases } => {
                output.push_str("enum ");
                output.push_str(&ty.name);
                output.push_str(" {\n");
                for case in cases {
                    output.push_str("    ");
                    output.push_str(&case.name);
                    output.push('\n');
                }
                output.push_str("}\n\n");
            }
            ComponentNamedTypeKind::Variant { cases } => {
                output.push_str("enum ");
                output.push_str(&ty.name);
                output.push_str(" {\n");
                for case in cases {
                    output.push_str("    ");
                    output.push_str(&case.name);
                    if let Some(payload) = &case.payload {
                        output.push('(');
                        write_restrict_value_type(&mut output, payload);
                        output.push(')');
                    }
                    output.push('\n');
                }
                output.push_str("}\n\n");
            }
        }
    }
    for binding in imports {
        output.push_str("fun ");
        output.push_str(&binding.local_function);
        output.push_str(": (");
        for (index, param) in binding.params.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(&format!("wit_arg_{index}: "));
            write_restrict_value_type(&mut output, &param.ty);
        }
        output.push_str(") -> ");
        if let Some(result) = &binding.result {
            write_restrict_value_type(&mut output, result);
        } else {
            output.push_str("()");
        }
        output.push_str(" = {\n    ");
        match binding.params.len() {
            0 => {
                output.push_str("() ");
                output.push_str(&binding.local_function);
            }
            1 => {
                output.push_str("wit_arg_0 |> ");
                output.push_str(&binding.local_function);
            }
            _ => {
                output.push('(');
                for index in 0..binding.params.len() {
                    if index > 0 {
                        output.push_str(", ");
                    }
                    output.push_str(&format!("wit_arg_{index}"));
                }
                output.push_str(") ");
                output.push_str(&binding.local_function);
            }
        }
        output.push_str("\n}\n\n");
    }
    Ok(output)
}

fn stable_restrict_import_name(binding: &ExternalWitFunctionBinding) -> String {
    let readable_name = binding
        .name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let digest = Sha256::digest(binding.canonical_identity.as_bytes());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut suffix = String::with_capacity(24);
    for byte in &digest[..12] {
        suffix.push(char::from(HEX[usize::from(byte >> 4)]));
        suffix.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    format!("wit_import_{readable_name}_{suffix}")
}

fn write_restrict_value_type(output: &mut String, ty: &ComponentValueType) {
    match ty {
        ComponentValueType::Unit => output.push_str("()"),
        ComponentValueType::S32 => output.push_str("Int32"),
        ComponentValueType::S64 => output.push_str("Int64"),
        ComponentValueType::F64 => output.push_str("Float64"),
        ComponentValueType::Bool => output.push_str("Boolean"),
        ComponentValueType::Char => output.push_str("Char"),
        ComponentValueType::String => output.push_str("String"),
        ComponentValueType::List(element) => {
            output.push_str("List<");
            write_restrict_value_type(output, element);
            output.push('>');
        }
        ComponentValueType::Array { element, length } => {
            output.push_str("Array<");
            write_restrict_value_type(output, element);
            output.push_str(&format!(", {length}>"));
        }
        ComponentValueType::Option(inner) => {
            output.push_str("Option<");
            write_restrict_value_type(output, inner);
            output.push('>');
        }
        ComponentValueType::Result { ok, err } => {
            output.push_str("Result<");
            write_restrict_value_type(output, ok);
            output.push_str(", ");
            write_restrict_value_type(output, err);
            output.push('>');
        }
        ComponentValueType::Named(name) => output.push_str(name),
        ComponentValueType::OwnResource(_) | ComponentValueType::OwnCallable(_) => {
            output.push_str("__UnsupportedExternalComponentHandle")
        }
    }
}

fn collect_world_functions<'a>(
    resolve: &Resolve,
    world_identity: &str,
    direction: ExternalWitDirection,
    items: impl IntoIterator<Item = (&'a WorldKey, &'a WorldItem)>,
    lowerer: &mut ExternalWitTypeLowerer<'_>,
) -> Result<Vec<ExternalWitFunctionBinding>, ExternalWitError> {
    let mut bindings = BTreeMap::new();
    for (key, item) in items {
        match item {
            WorldItem::Function(function) => {
                let binding = lower_function(world_identity, direction, None, function, lowerer)?;
                bindings.insert(binding.canonical_identity.clone(), binding);
            }
            WorldItem::Interface { id, .. } => {
                validate_interface_types(resolve, world_identity, direction, key, *id, lowerer)?;
                let interface_identity = stable_interface_identity(resolve, world_identity, key);
                for function in resolve.interfaces[*id].functions.values() {
                    let binding = lower_function(
                        world_identity,
                        direction,
                        Some(&interface_identity),
                        function,
                        lowerer,
                    )?;
                    bindings.insert(binding.canonical_identity.clone(), binding);
                }
            }
            WorldItem::Type { id, .. } => {
                let boundary = format!(
                    "{world_identity} {} type {}",
                    direction.label(),
                    resolve.name_world_key(key)
                );
                lowerer.lower_declared_type(*id, &boundary)?;
            }
        }
    }
    Ok(bindings.into_values().collect())
}

fn validate_interface_types(
    resolve: &Resolve,
    world_identity: &str,
    direction: ExternalWitDirection,
    key: &WorldKey,
    interface_id: InterfaceId,
    lowerer: &mut ExternalWitTypeLowerer<'_>,
) -> Result<(), ExternalWitError> {
    let interface_identity = stable_interface_identity(resolve, world_identity, key);
    for (name, type_id) in &resolve.interfaces[interface_id].types {
        let boundary = format!(
            "{world_identity} {} {interface_identity} type {name}",
            direction.label()
        );
        lowerer.lower_declared_type(*type_id, &boundary)?;
    }
    Ok(())
}

fn lower_function(
    world_identity: &str,
    direction: ExternalWitDirection,
    interface: Option<&str>,
    function: &Function,
    lowerer: &mut ExternalWitTypeLowerer<'_>,
) -> Result<ExternalWitFunctionBinding, ExternalWitError> {
    let canonical_identity =
        canonical_function_identity(world_identity, direction, interface, function);
    if function.kind.is_async() {
        return Err(ExternalWitError::UnsupportedAsync {
            boundary: canonical_identity,
        });
    }
    if !matches!(function.kind, FunctionKind::Freestanding) {
        return Err(ExternalWitError::UnsupportedResource {
            boundary: canonical_identity,
        });
    }

    let params = function
        .params
        .iter()
        .map(|param| {
            let boundary = format!("{canonical_identity} parameter {}", param.name);
            let path = format!("{canonical_identity}/param/{}", param.name);
            lowerer
                .lower_type(&param.ty, &path, &boundary)
                .map(|ty| ExternalWitParam {
                    name: param.name.clone(),
                    ty,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = function
        .result
        .as_ref()
        .map(|ty| {
            let boundary = format!("{canonical_identity} result");
            let path = format!("{canonical_identity}/result");
            lowerer.lower_type(ty, &path, &boundary)
        })
        .transpose()?;

    Ok(ExternalWitFunctionBinding {
        canonical_identity,
        direction,
        interface: interface.map(str::to_string),
        name: function.name.clone(),
        params,
        result,
    })
}

fn canonical_function_identity(
    world_identity: &str,
    direction: ExternalWitDirection,
    interface: Option<&str>,
    function: &Function,
) -> String {
    let namespace = interface.unwrap_or("$world");
    format!(
        "{world_identity}#{}{namespace}/{}",
        direction.label(),
        function.name
    )
}

struct ExternalWitTypeLowerer<'a> {
    resolve: &'a Resolve,
    array_lengths: BTreeMap<String, u32>,
    used_array_sites: BTreeSet<String>,
    list_sites: BTreeMap<String, ExternalWitListSite>,
    declarations: BTreeMap<String, ComponentNamedTypeIr>,
    named_values: HashMap<TypeId, ComponentValueType>,
    visiting: HashSet<TypeId>,
    local_type_owners: BTreeMap<String, String>,
}

impl<'a> ExternalWitTypeLowerer<'a> {
    fn new(resolve: &'a Resolve, array_lengths: BTreeMap<String, u32>) -> Self {
        Self {
            resolve,
            array_lengths,
            used_array_sites: BTreeSet::new(),
            list_sites: BTreeMap::new(),
            declarations: BTreeMap::new(),
            named_values: HashMap::new(),
            visiting: HashSet::new(),
            local_type_owners: BTreeMap::new(),
        }
    }

    fn finish(
        self,
    ) -> Result<(Vec<ComponentNamedTypeIr>, Vec<ExternalWitListSite>), ExternalWitError> {
        if let Some(site) = self
            .array_lengths
            .keys()
            .find(|site| !self.used_array_sites.contains(*site))
        {
            return Err(ExternalWitError::UnknownArrayMetadata { site: site.clone() });
        }
        Ok((
            self.declarations.into_values().collect(),
            self.list_sites.into_values().collect(),
        ))
    }

    fn lower_declared_type(
        &mut self,
        type_id: TypeId,
        boundary: &str,
    ) -> Result<(), ExternalWitError> {
        self.lower_type_id(type_id, boundary, boundary).map(|_| ())
    }

    fn lower_type(
        &mut self,
        ty: &Type,
        path: &str,
        boundary: &str,
    ) -> Result<ComponentValueType, ExternalWitError> {
        match ty {
            Type::S32 => Ok(ComponentValueType::S32),
            Type::S64 => Ok(ComponentValueType::S64),
            Type::F64 => Ok(ComponentValueType::F64),
            Type::Bool => Ok(ComponentValueType::Bool),
            Type::Char => Ok(ComponentValueType::Char),
            Type::String => Ok(ComponentValueType::String),
            Type::Id(id) => self.lower_type_id(*id, path, boundary),
            Type::ErrorContext => unsupported_primitive(boundary, "error-context"),
            Type::U8 => unsupported_primitive(boundary, "u8"),
            Type::U16 => unsupported_primitive(boundary, "u16"),
            Type::U32 => unsupported_primitive(boundary, "u32"),
            Type::U64 => unsupported_primitive(boundary, "u64"),
            Type::S8 => unsupported_primitive(boundary, "s8"),
            Type::S16 => unsupported_primitive(boundary, "s16"),
            Type::F32 => unsupported_primitive(boundary, "f32"),
        }
    }

    fn lower_type_id(
        &mut self,
        type_id: TypeId,
        caller_path: &str,
        boundary: &str,
    ) -> Result<ComponentValueType, ExternalWitError> {
        let named = self.named_type_identity(type_id, boundary)?;
        if named.is_some() {
            if let Some(value) = self.named_values.get(&type_id) {
                return Ok(value.clone());
            }
        }
        if !self.visiting.insert(type_id) {
            return Err(ExternalWitError::TypeAliasCycle {
                boundary: boundary.to_string(),
            });
        }
        let path = named
            .as_ref()
            .map(|(identity, _)| identity.as_str())
            .unwrap_or(caller_path);
        let kind = self.resolve.types[type_id].kind.clone();
        let result = match kind {
            TypeDefKind::Type(inner) => self.lower_type(&inner, path, boundary),
            TypeDefKind::List(element) => self.lower_list(&element, path, boundary),
            TypeDefKind::FixedLengthList(element, length) => {
                let element = self.lower_type(&element, &format!("{path}/element"), boundary)?;
                self.list_sites.insert(
                    path.to_string(),
                    ExternalWitListSite {
                        canonical_path: path.to_string(),
                        exact_length: Some(length),
                    },
                );
                Ok(ComponentValueType::Array {
                    element: Box::new(element),
                    length: length as usize,
                })
            }
            TypeDefKind::Option(inner) => Ok(ComponentValueType::Option(Box::new(
                self.lower_type(&inner, &format!("{path}/some"), boundary)?,
            ))),
            TypeDefKind::Result(result) => Ok(ComponentValueType::Result {
                ok: Box::new(
                    result
                        .ok
                        .as_ref()
                        .map(|ty| self.lower_type(ty, &format!("{path}/ok"), boundary))
                        .transpose()?
                        .unwrap_or(ComponentValueType::Unit),
                ),
                err: Box::new(
                    result
                        .err
                        .as_ref()
                        .map(|ty| self.lower_type(ty, &format!("{path}/err"), boundary))
                        .transpose()?
                        .unwrap_or(ComponentValueType::Unit),
                ),
            }),
            TypeDefKind::Record(record) => {
                self.lower_record(type_id, named.as_ref(), record, boundary)
            }
            TypeDefKind::Enum(enum_) => self.lower_enum(type_id, named.as_ref(), enum_, boundary),
            TypeDefKind::Variant(variant) => {
                self.lower_variant(type_id, named.as_ref(), variant, boundary)
            }
            TypeDefKind::Resource | TypeDefKind::Handle(_) => {
                Err(ExternalWitError::UnsupportedResource {
                    boundary: boundary.to_string(),
                })
            }
            TypeDefKind::Future(_) | TypeDefKind::Stream(_) => {
                Err(ExternalWitError::UnsupportedAsync {
                    boundary: boundary.to_string(),
                })
            }
            kind => Err(ExternalWitError::UnsupportedType {
                boundary: boundary.to_string(),
                kind: kind.as_str(),
            }),
        };
        self.visiting.remove(&type_id);
        if named.is_some() {
            if let Ok(value) = &result {
                self.named_values.insert(type_id, value.clone());
            }
        }
        result
    }

    fn lower_list(
        &mut self,
        element: &Type,
        path: &str,
        boundary: &str,
    ) -> Result<ComponentValueType, ExternalWitError> {
        let exact_length = self.array_lengths.get(path).copied();
        if exact_length.is_some() {
            self.used_array_sites.insert(path.to_string());
        }
        self.list_sites.insert(
            path.to_string(),
            ExternalWitListSite {
                canonical_path: path.to_string(),
                exact_length,
            },
        );
        let element = self.lower_type(element, &format!("{path}/element"), boundary)?;
        Ok(match exact_length {
            Some(length) => ComponentValueType::Array {
                element: Box::new(element),
                length: length as usize,
            },
            None => ComponentValueType::List(Box::new(element)),
        })
    }

    fn lower_record(
        &mut self,
        type_id: TypeId,
        named: Option<&(String, String)>,
        record: wit_parser::Record,
        boundary: &str,
    ) -> Result<ComponentValueType, ExternalWitError> {
        let (identity, wit_name) =
            named.ok_or_else(|| ExternalWitError::InvalidNamedTypeIdentity {
                boundary: boundary.to_string(),
            })?;
        let local_name = self.local_type_name(identity, wit_name)?;
        let mut names = BTreeSet::new();
        let mut fields = Vec::with_capacity(record.fields.len());
        for (source_index, field) in record.fields.into_iter().enumerate() {
            let name = restrict_member_name("wit_field", &field.name);
            if !names.insert(name.clone()) {
                return Err(ExternalWitError::RestrictNameCollision {
                    boundary: identity.clone(),
                    name,
                });
            }
            fields.push(ComponentRecordFieldIr {
                source_index,
                source_name: field.name.clone(),
                name,
                ty: self.lower_type(
                    &field.ty,
                    &format!("{identity}/field/{}", field.name),
                    boundary,
                )?,
            });
        }
        self.declarations.insert(
            identity.clone(),
            ComponentNamedTypeIr {
                name: local_name.clone(),
                source_type: identity.clone(),
                kind: ComponentNamedTypeKind::Record { fields },
            },
        );
        let value = ComponentValueType::Named(local_name);
        self.named_values.insert(type_id, value.clone());
        Ok(value)
    }

    fn lower_enum(
        &mut self,
        type_id: TypeId,
        named: Option<&(String, String)>,
        enum_: wit_parser::Enum,
        boundary: &str,
    ) -> Result<ComponentValueType, ExternalWitError> {
        let (identity, wit_name) =
            named.ok_or_else(|| ExternalWitError::InvalidNamedTypeIdentity {
                boundary: boundary.to_string(),
            })?;
        let local_name = self.local_type_name(identity, wit_name)?;
        let mut names = BTreeSet::new();
        let mut cases = Vec::with_capacity(enum_.cases.len());
        for (source_index, case) in enum_.cases.into_iter().enumerate() {
            let name = restrict_member_name("WitCase", &case.name);
            if !names.insert(name.clone()) {
                return Err(ExternalWitError::RestrictNameCollision {
                    boundary: identity.clone(),
                    name,
                });
            }
            cases.push(ComponentEnumCaseIr {
                source_index,
                source_name: case.name,
                name,
            });
        }
        self.declarations.insert(
            identity.clone(),
            ComponentNamedTypeIr {
                name: local_name.clone(),
                source_type: identity.clone(),
                kind: ComponentNamedTypeKind::Enum { cases },
            },
        );
        let value = ComponentValueType::Named(local_name);
        self.named_values.insert(type_id, value.clone());
        Ok(value)
    }

    fn lower_variant(
        &mut self,
        type_id: TypeId,
        named: Option<&(String, String)>,
        variant: wit_parser::Variant,
        boundary: &str,
    ) -> Result<ComponentValueType, ExternalWitError> {
        let (identity, wit_name) =
            named.ok_or_else(|| ExternalWitError::InvalidNamedTypeIdentity {
                boundary: boundary.to_string(),
            })?;
        let local_name = self.local_type_name(identity, wit_name)?;
        let mut names = BTreeSet::new();
        let mut cases = Vec::with_capacity(variant.cases.len());
        for (source_index, case) in variant.cases.into_iter().enumerate() {
            let name = restrict_member_name("WitCase", &case.name);
            if !names.insert(name.clone()) {
                return Err(ExternalWitError::RestrictNameCollision {
                    boundary: identity.clone(),
                    name,
                });
            }
            let payload = case
                .ty
                .as_ref()
                .map(|ty| self.lower_type(ty, &format!("{identity}/case/{}", case.name), boundary))
                .transpose()?;
            cases.push(ComponentVariantCaseIr {
                source_index,
                source_name: case.name,
                name,
                payload,
            });
        }
        self.declarations.insert(
            identity.clone(),
            ComponentNamedTypeIr {
                name: local_name.clone(),
                source_type: identity.clone(),
                kind: ComponentNamedTypeKind::Variant { cases },
            },
        );
        let value = ComponentValueType::Named(local_name);
        self.named_values.insert(type_id, value.clone());
        Ok(value)
    }

    fn named_type_identity(
        &self,
        type_id: TypeId,
        boundary: &str,
    ) -> Result<Option<(String, String)>, ExternalWitError> {
        let definition = &self.resolve.types[type_id];
        let Some(name) = definition.name.clone() else {
            return Ok(None);
        };
        let owner = match definition.owner {
            TypeOwner::Interface(interface) => self.resolve.id_of(interface),
            TypeOwner::World(world) => {
                let world = &self.resolve.worlds[world];
                world
                    .package
                    .map(|package| self.resolve.id_of_name(package, &world.name))
            }
            TypeOwner::None => None,
        }
        .ok_or_else(|| ExternalWitError::InvalidNamedTypeIdentity {
            boundary: boundary.to_string(),
        })?;
        Ok(Some((format!("{owner}/type/{name}"), name)))
    }

    fn local_type_name(
        &mut self,
        identity: &str,
        wit_name: &str,
    ) -> Result<String, ExternalWitError> {
        let name = format!(
            "WitType_{}_{}",
            readable_restrict_name(wit_name),
            stable_identity_suffix(identity)
        );
        if let Some(first) = self
            .local_type_owners
            .insert(name.clone(), identity.to_string())
        {
            if first != identity {
                return Err(ExternalWitError::RestrictNameCollision {
                    boundary: identity.to_string(),
                    name,
                });
            }
        }
        Ok(name)
    }
}

fn unsupported_primitive<T>(boundary: &str, kind: &'static str) -> Result<T, ExternalWitError> {
    Err(ExternalWitError::UnsupportedType {
        boundary: boundary.to_string(),
        kind,
    })
}

fn readable_restrict_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn restrict_member_name(prefix: &str, value: &str) -> String {
    format!("{prefix}_{}", readable_restrict_name(value))
}

fn stable_identity_suffix(identity: &str) -> String {
    let digest = Sha256::digest(identity.as_bytes());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut suffix = String::with_capacity(24);
    for byte in &digest[..12] {
        suffix.push(char::from(HEX[usize::from(byte >> 4)]));
        suffix.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    suffix
}

fn stable_interface_identity(resolve: &Resolve, world_identity: &str, key: &WorldKey) -> String {
    match key {
        WorldKey::Interface(id) => resolve
            .id_of(*id)
            .unwrap_or_else(|| format!("{world_identity}/$anonymous-interface")),
        WorldKey::Name(name) => format!("{world_identity}/{name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALAR_WIT: &str = r#"
        package external:calculator@1.2.3;

        interface math {
            z-last: func(value: s64) -> f64;
            a-first: func(left: s32, ready: bool, marker: char) -> s32;
        }

        world consumer {
            import math;
            import ping: func();
        }
    "#;

    #[test]
    fn resolves_external_scalar_imports_deterministically() {
        let first = ExternalWitContract::from_text("calculator.wit", SCALAR_WIT, "consumer")
            .expect("scalar WIT should resolve");
        let second = ExternalWitContract::from_text("renamed.wit", SCALAR_WIT, "consumer")
            .expect("logical source path must not change semantic identity");
        let ir = first.binding_ir();

        assert_eq!(ir.authority, ComponentInterfaceAuthority::ExternalWit);
        assert_eq!(ir.package, "external:calculator@1.2.3");
        assert_eq!(
            ir.canonical_world_identity,
            "external:calculator/consumer@1.2.3"
        );
        assert_eq!(ir.contract_hash, second.binding_ir().contract_hash);
        assert_eq!(
            ir.imports
                .iter()
                .map(|binding| binding.name.as_str())
                .collect::<Vec<_>>(),
            vec!["ping", "a-first", "z-last"]
        );
        assert!(ir.exports.is_empty());
        assert_eq!(ir.imports[0].result, None);
        assert_eq!(ir.imports[1].params[0].ty, ComponentValueType::S32);
        assert_eq!(ir.imports[1].params[1].ty, ComponentValueType::Bool);
        assert_eq!(ir.imports[1].params[2].ty, ComponentValueType::Char);
        assert_eq!(ir.imports[2].result, Some(ComponentValueType::F64));

        let plan = first.restrict_binding_plan();
        assert_eq!(
            plan.schema_version,
            EXTERNAL_WIT_RESTRICT_BINDING_PLAN_SCHEMA_VERSION
        );
        assert_eq!(plan.contract_hash, ir.contract_hash);
        assert_eq!(
            plan.imports[0].local_function,
            "wit_import_ping_3944f1b60f47fdae3484f8be"
        );
        assert_eq!(plan.imports[0].core_module, "cm32p2");
        assert_eq!(plan.imports[0].core_name, "ping");
        assert!(plan.stub_source.contains(
            "fun wit_import_a_first_1f964630a2be1150a5dcc5d4: (wit_arg_0: Int32, wit_arg_1: Boolean, wit_arg_2: Char) -> Int32"
        ));
        assert_eq!(plan, second.restrict_binding_plan());
    }

    #[test]
    fn resolves_a_wit_directory() {
        let directory = tempfile::tempdir().expect("temporary WIT directory");
        std::fs::write(directory.path().join("world.wit"), SCALAR_WIT).expect("write WIT fixture");
        let contract = ExternalWitContract::from_path(directory.path(), "consumer")
            .expect("directory WIT should resolve");
        assert_eq!(contract.binding_ir().imports.len(), 3);
    }

    #[test]
    fn generated_import_names_survive_additive_contract_changes() {
        let original = ExternalWitContract::from_text(
            "stable.wit",
            r#"
                package external:stable@1.0.0;
                interface api { existing: func(value: s32) -> s32; }
                world consumer { import api; }
            "#,
            "consumer",
        )
        .expect("original contract should resolve");
        let extended = ExternalWitContract::from_text(
            "stable.wit",
            r#"
                package external:stable@1.0.0;
                interface api {
                    added: func(value: s64) -> s64;
                    existing: func(value: s32) -> s32;
                }
                world consumer { import api; }
            "#,
            "consumer",
        )
        .expect("extended contract should resolve");

        let original_existing = original
            .restrict_binding_plan()
            .imports
            .iter()
            .find(|binding| binding.name == "existing")
            .expect("original binding should exist");
        let extended_existing = extended
            .restrict_binding_plan()
            .imports
            .iter()
            .find(|binding| binding.name == "existing")
            .expect("extended binding should exist");
        assert_eq!(
            original_existing.local_function,
            extended_existing.local_function
        );
        assert_ne!(
            original.binding_ir().contract_hash,
            extended.binding_ir().contract_hash
        );
    }

    #[test]
    fn resolves_an_encoded_wit_package() {
        let text_contract =
            ExternalWitContract::from_text("calculator.wit", SCALAR_WIT, "consumer")
                .expect("text WIT should resolve");
        let directory = tempfile::tempdir().expect("temporary WIT package directory");
        let package_path = directory.path().join("calculator.wasm");
        std::fs::write(&package_path, text_contract.encoded_wit_package())
            .expect("write encoded WIT package");

        let encoded_contract = ExternalWitContract::from_path(&package_path, "consumer")
            .expect("encoded WIT package should resolve");
        assert_eq!(
            encoded_contract.binding_ir().canonical_world_identity,
            text_contract.binding_ir().canonical_world_identity
        );
        assert_eq!(
            encoded_contract.binding_ir().contract_hash,
            text_contract.binding_ir().contract_hash
        );
    }

    #[test]
    fn requires_an_explicit_world() {
        let error = ExternalWitContract::from_text("empty.wit", SCALAR_WIT, "")
            .expect_err("empty world selector must fail");
        assert!(matches!(error, ExternalWitError::MissingWorldSelection));
        assert_eq!(error.stage(), "world-selection");
    }

    #[test]
    fn rejects_async_before_binding_generation() {
        let error = ExternalWitContract::from_text(
            "async.wit",
            "package external:async-api@1.0.0; world consumer { import wait: async func(); }",
            "consumer",
        )
        .expect_err("v0.1 must reject async WIT");
        assert!(matches!(error, ExternalWitError::UnsupportedAsync { .. }));
        assert_eq!(error.stage(), "binding-ir");
    }

    #[test]
    fn projects_string_bindings_for_guest_import_adapter_generation() {
        let contract = ExternalWitContract::from_text(
            "string.wit",
            "package external:text@1.0.0; world consumer { import echo: func(value: string) -> string; }",
            "consumer",
        )
        .expect("synchronous string WIT should produce a typed plan");
        let binding = &contract.binding_ir().imports[0];
        assert_eq!(binding.params[0].ty, ComponentValueType::String);
        assert_eq!(binding.result, Some(ComponentValueType::String));
        assert_eq!(
            contract.restrict_binding_plan().rich_features,
            vec![ExternalWitRichFeature::String]
        );
        assert!(contract
            .restrict_binding_plan()
            .stub_source
            .contains("wit_arg_0: String) -> String"));
    }

    #[test]
    fn rejects_resources_even_when_no_method_is_called() {
        let error = ExternalWitContract::from_text(
            "resource.wit",
            r#"
                package external:handles@1.0.0;
                interface handles { resource file; }
                world consumer { import handles; }
            "#,
            "consumer",
        )
        .expect_err("external value adapter must reject resource declarations");
        assert!(matches!(
            error,
            ExternalWitError::UnsupportedResource { .. }
        ));
    }

    #[test]
    fn rejects_unmapped_bytes_and_async_value_types_fail_closed() {
        let bytes = ExternalWitContract::from_text(
            "bytes.wit",
            r#"
                package external:bytes@1.0.0;
                interface api {
                    consume: func(value: list<u8>);
                }
                world consumer { import api; }
            "#,
            "consumer",
        )
        .expect_err("UInt8/Bytes has no accepted Restrict source representation yet");
        assert!(matches!(
            bytes,
            ExternalWitError::UnsupportedType { kind: "u8", .. }
        ));

        let future = ExternalWitContract::from_text(
            "future.wit",
            r#"
                package external:futures@1.0.0;
                interface api {
                    type pending = future<string>;
                    consume: func(value: pending);
                }
                world consumer { import api; }
            "#,
            "consumer",
        )
        .expect_err("future values belong to v0.2");
        assert!(matches!(future, ExternalWitError::UnsupportedAsync { .. }));
    }

    const RICH_WIT: &str = r#"
        package external:rich-values@1.0.0;

        interface api {
            record profile {
                name: string,
                tags: list<string>,
            }
            enum state { ready, stopped }
            variant event {
                idle,
                changed(profile),
                failed(string),
            }
            type maybe-profile = option<profile>;
            type profile-result = result<profile, string>;
            transform: func(
                profiles: list<profile>,
                maybe: maybe-profile,
                outcome: profile-result,
                current: state,
                event: event,
            ) -> event;
        }

        world consumer {
            import api;
        }
    "#;

    #[test]
    fn rich_value_graph_preserves_nominal_member_order_and_typed_stubs() {
        let first = ExternalWitContract::from_text("rich.wit", RICH_WIT, "consumer")
            .expect("rich synchronous WIT should resolve");
        let second = ExternalWitContract::from_text("renamed.wit", RICH_WIT, "consumer")
            .expect("logical WIT path must not affect the rich plan");
        let ir = first.binding_ir();
        assert_eq!(ir, second.binding_ir());
        assert_eq!(ir.types.len(), 3);
        assert_eq!(ir.list_sites.len(), 2);
        assert!(ir.list_sites.iter().all(|site| site.exact_length.is_none()));

        let profile = ir
            .types
            .iter()
            .find(|ty| ty.source_type.ends_with("/type/profile"))
            .expect("record declaration should be retained");
        let ComponentNamedTypeKind::Record { fields } = &profile.kind else {
            panic!("profile should lower as a record");
        };
        assert_eq!(
            fields
                .iter()
                .map(|field| field.source_name.as_str())
                .collect::<Vec<_>>(),
            vec!["name", "tags"]
        );

        let event = ir
            .types
            .iter()
            .find(|ty| ty.source_type.ends_with("/type/event"))
            .expect("variant declaration should be retained");
        let ComponentNamedTypeKind::Variant { cases } = &event.kind else {
            panic!("event should lower as a variant");
        };
        assert_eq!(
            cases
                .iter()
                .map(|case| case.source_name.as_str())
                .collect::<Vec<_>>(),
            vec!["idle", "changed", "failed"]
        );

        assert_eq!(
            first.restrict_binding_plan().rich_features,
            vec![
                ExternalWitRichFeature::String,
                ExternalWitRichFeature::List,
                ExternalWitRichFeature::Record,
                ExternalWitRichFeature::Option,
                ExternalWitRichFeature::Result,
                ExternalWitRichFeature::Enum,
                ExternalWitRichFeature::Variant,
            ]
        );
        assert_eq!(
            first.restrict_binding_plan(),
            second.restrict_binding_plan()
        );
        let (remaining, stubs) = parse_program(&first.restrict_binding_plan().stub_source)
            .expect("generated rich declarations and stubs should parse");
        assert!(remaining.trim().is_empty());
        let mut checker = TypeChecker::new();
        checker
            .check_program(&stubs)
            .expect("generated rich bindings should type-check before adapter lowering");
    }

    #[test]
    fn explicit_array_metadata_refines_only_the_named_list_site() {
        let wit = r#"
            package external:arrays@1.0.0;
            interface api {
                type values = list<string>;
                echo: func(value: values) -> values;
            }
            world consumer { import api; }
        "#;
        let ordinary = ExternalWitContract::from_text("arrays.wit", wit, "consumer")
            .expect("ordinary list contract should resolve");
        assert_eq!(ordinary.binding_ir().list_sites.len(), 1);
        let site = ordinary.binding_ir().list_sites[0].canonical_path.clone();
        let array = ExternalWitContract::from_text_with_metadata(
            "arrays.wit",
            wit,
            "consumer",
            ExternalWitAdapterMetadata {
                array_lengths: vec![ExternalWitArrayLengthMetadata::new(site.clone(), 3)],
            },
        )
        .expect("explicit list-site metadata should refine the Restrict type");

        assert_eq!(
            ordinary.binding_ir().contract_hash,
            array.binding_ir().contract_hash
        );
        assert_ne!(
            ordinary.binding_ir().adapter_metadata_hash,
            array.binding_ir().adapter_metadata_hash
        );
        assert_eq!(array.binding_ir().list_sites[0].exact_length, Some(3));
        assert_eq!(
            array.binding_ir().imports[0].params[0].ty,
            ComponentValueType::Array {
                element: Box::new(ComponentValueType::String),
                length: 3,
            }
        );
        assert_eq!(
            array.binding_ir().imports[0].result,
            Some(ComponentValueType::Array {
                element: Box::new(ComponentValueType::String),
                length: 3,
            })
        );
        assert!(array
            .restrict_binding_plan()
            .stub_source
            .contains("Array<String, 3>"));

        let duplicate = ExternalWitContract::from_text_with_metadata(
            "arrays.wit",
            wit,
            "consumer",
            ExternalWitAdapterMetadata {
                array_lengths: vec![
                    ExternalWitArrayLengthMetadata::new(site.clone(), 3),
                    ExternalWitArrayLengthMetadata::new(site, 4),
                ],
            },
        )
        .expect_err("duplicate array refinements must fail closed");
        assert!(matches!(
            duplicate,
            ExternalWitError::DuplicateArrayMetadata { .. }
        ));

        let unknown = ExternalWitContract::from_text_with_metadata(
            "arrays.wit",
            wit,
            "consumer",
            ExternalWitAdapterMetadata {
                array_lengths: vec![ExternalWitArrayLengthMetadata::new("missing/site", 3)],
            },
        )
        .expect_err("unknown array refinements must fail closed");
        assert!(matches!(
            unknown,
            ExternalWitError::UnknownArrayMetadata { .. }
        ));
    }

    #[test]
    fn rich_external_export_stays_fail_closed_at_its_distinct_adapter_seam() {
        let contract = ExternalWitContract::from_text(
            "rich-consumer.wit",
            r#"
                package external:rich-consumer@1.0.0;
                interface api {
                    record profile { name: string }
                    echo: func(value: profile) -> profile;
                }
                world consumer {
                    import api;
                    use api.{profile};
                    export run: func(value: profile) -> profile;
                }
            "#,
            "consumer",
        )
        .expect("rich consumer world should resolve");
        let local_type = contract.binding_ir().types[0].name.clone();
        let local_import = contract.restrict_binding_plan().imports[0]
            .local_function
            .clone();
        let source = format!(
            "pub fun run: (value: {local_type}) -> {local_type} = {{ value |> {local_import} }}"
        );
        let implementations = [ExternalWitExportImplementation::new(
            contract.restrict_binding_plan().exports[0]
                .canonical_identity
                .clone(),
            "run",
        )];
        let error = compile_external_wit_component(ExternalWitCompileRequest {
            source: &source,
            entry_path: Path::new("rich-consumer.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &implementations,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect_err("rich external export should stop at its guest-export adapter seam");
        assert!(matches!(
            error,
            ExternalWitCompileError::CanonicalAdapter(ref message)
                if message.contains("guest-export rich Canonical ABI adapter")
        ));
        assert_eq!(error.stage(), "core-lowering");

        let mismatch = compile_external_wit_component(ExternalWitCompileRequest {
            source: "pub fun run: (value: String) -> String = { value }",
            entry_path: Path::new("rich-consumer.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &implementations,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect_err("rich implementation signatures must match before adapter lowering");
        assert!(matches!(
            mismatch,
            ExternalWitCompileError::ExportSignatureMismatch { .. }
        ));
    }

    #[test]
    fn rich_string_imports_compile_through_real_guest_import_adapters() {
        let contract = ExternalWitContract::from_text(
            "rich-imports.wit",
            r#"
                package external:rich-imports@1.0.0;
                interface api {
                    make: func() -> string;
                    length: func(value: string) -> s32;
                }
                world consumer {
                    import api;
                    export run: func() -> s32;
                }
            "#,
            "consumer",
        )
        .expect("rich import contract should resolve");
        let make = contract
            .restrict_binding_plan()
            .imports
            .iter()
            .find(|binding| binding.name == "make")
            .expect("make binding")
            .local_function
            .clone();
        let length = contract
            .restrict_binding_plan()
            .imports
            .iter()
            .find(|binding| binding.name == "length")
            .expect("length binding")
            .local_function
            .clone();
        let source = format!("pub fun run: () -> Int32 = {{ (() {make}) |> {length} }}");
        let implementations = [ExternalWitExportImplementation::new(
            contract.restrict_binding_plan().exports[0]
                .canonical_identity
                .clone(),
            "run",
        )];
        let artifact = compile_external_wit_component(ExternalWitCompileRequest {
            source: &source,
            entry_path: Path::new("rich-imports.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &implementations,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x4000,
        })
        .expect("rich string imports should componentize through guest-import adapters");
        contract
            .validate_component(&artifact.component_wasm)
            .expect("rich import consumer should conform to authoritative WIT");
    }

    #[test]
    fn rich_generated_nominal_name_collision_fails_before_type_checking() {
        let contract = ExternalWitContract::from_text(
            "collision.wit",
            r#"
                package external:collision@1.0.0;
                interface api {
                    record profile { name: string }
                    echo: func(value: profile) -> profile;
                }
                world consumer { import api; }
            "#,
            "consumer",
        )
        .expect("rich contract should resolve");
        let generated = contract.binding_ir().types[0].name.clone();
        let source = format!("record {generated} {{ value: Int32 }}");
        let error = compile_external_wit_component(ExternalWitCompileRequest {
            source: &source,
            entry_path: Path::new("collision.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &[],
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect_err("generated nominal names must not shadow source declarations");
        assert!(matches!(
            error,
            ExternalWitCompileError::GeneratedBindingCollision(name) if name == generated
        ));
    }

    #[test]
    fn validates_a_genuinely_encoded_component_against_external_wit() {
        let contract = ExternalWitContract::from_text(
            "producer.wit",
            "package external:producer@1.0.0; world producer { export answer: func() -> s32; }",
            "producer",
        )
        .expect("producer contract should resolve");
        assert_eq!(contract.binding_ir().exports.len(), 1);
        assert_eq!(
            contract.binding_ir().exports[0].direction,
            ExternalWitDirection::Export
        );
        let component = wat::parse_str(
            r#"
                (component
                    (core module $producer
                        (func (export "answer") (result i32)
                            i32.const 42))
                    (core instance $instance (instantiate $producer))
                    (func $answer (result s32)
                        (canon lift (core func $instance "answer")))
                    (export "answer" (func $answer)))
            "#,
        )
        .expect("dummy Rust-style producer Component should encode");

        let report = contract
            .validate_component(&component)
            .expect("component should target external producer world");
        assert_eq!(
            report.canonical_world_identity,
            "external:producer/producer@1.0.0"
        );
        assert_eq!(report.contract_hash, contract.binding_ir().contract_hash);
    }

    #[test]
    fn rejects_a_component_with_a_mismatched_export_type() {
        let contract = ExternalWitContract::from_text(
            "producer.wit",
            "package external:producer@1.0.0; world producer { export answer: func() -> s64; }",
            "producer",
        )
        .expect("producer contract should resolve");
        let component = wat::parse_str(
            r#"
                (component
                    (core module $producer
                        (func (export "answer") (result i32)
                            i32.const 42))
                    (core instance $instance (instantiate $producer))
                    (func $answer (result s32)
                        (canon lift (core func $instance "answer")))
                    (export "answer" (func $answer)))
            "#,
        )
        .expect("component should encode");
        let error = contract
            .validate_component(&component)
            .expect_err("component must not target a different result type");
        assert!(matches!(
            error,
            ExternalWitError::ComponentConformance { .. }
        ));
        assert_eq!(error.stage(), "component-conformance");
    }

    #[test]
    fn rejects_core_wasm_before_conformance() {
        let contract = ExternalWitContract::from_text(
            "empty.wit",
            "package external:empty@1.0.0; world empty {}",
            "empty",
        )
        .expect("empty world should resolve");
        let core = wat::parse_str("(module)").expect("core module should encode");
        assert!(matches!(
            contract.validate_component(&core),
            Err(ExternalWitError::ComponentRequired)
        ));
    }

    const CONSUMER_WIT: &str = r#"
        package external:runtime@1.0.0;

        interface math {
            increment: func(value: s32) -> s32;
        }

        world consumer {
            import math;
            export run: func(value: s32) -> s32;
        }
    "#;

    #[test]
    fn compiles_external_scalar_world_with_generated_imports_deterministically() {
        let contract = ExternalWitContract::from_text("runtime.wit", CONSUMER_WIT, "consumer")
            .expect("consumer contract should resolve");
        let obligation = contract.restrict_binding_plan().exports[0]
            .canonical_identity
            .clone();
        let implementations = [ExternalWitExportImplementation::new(obligation, "run")];
        let source = format!(
            "pub fun run: (value: Int32) -> Int32 = {{ value |> {} }}",
            contract.restrict_binding_plan().imports[0].local_function
        );
        let request = || ExternalWitCompileRequest {
            source: &source,
            entry_path: Path::new("consumer.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &implementations,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        };

        let first = compile_external_wit_component(request())
            .expect("external scalar consumer should compile");
        let second = compile_external_wit_component(request())
            .expect("repeated external scalar consumer should compile");
        assert_eq!(first.component_wasm, second.component_wasm);
        assert_eq!(first.binding_plan, *contract.restrict_binding_plan());
        contract
            .validate_component(&first.component_wasm)
            .expect("generated consumer should target the external world");
    }

    #[test]
    fn external_world_requires_every_explicit_export_mapping() {
        let contract = ExternalWitContract::from_text("runtime.wit", CONSUMER_WIT, "consumer")
            .expect("consumer contract should resolve");
        let error = compile_external_wit_component(ExternalWitCompileRequest {
            source: "pub fun run: (value: Int32) -> Int32 = { value }",
            entry_path: Path::new("consumer.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &[],
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect_err("missing implementation mapping must fail");
        assert!(matches!(
            error,
            ExternalWitCompileError::MissingExportImplementations(_)
        ));
        assert_eq!(error.stage(), "implementation-conformance");
    }

    #[test]
    fn external_world_rejects_export_signature_mismatch() {
        let contract = ExternalWitContract::from_text("runtime.wit", CONSUMER_WIT, "consumer")
            .expect("consumer contract should resolve");
        let implementations = [ExternalWitExportImplementation::new(
            contract.restrict_binding_plan().exports[0]
                .canonical_identity
                .clone(),
            "run",
        )];
        let error = compile_external_wit_component(ExternalWitCompileRequest {
            source: "pub fun run: (value: Int64) -> Int64 = { value }",
            entry_path: Path::new("consumer.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &implementations,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect_err("mismatched implementation signature must fail");
        assert!(matches!(
            error,
            ExternalWitCompileError::ExportSignatureMismatch { .. }
        ));
    }

    #[test]
    fn external_world_rejects_generated_binding_name_collision() {
        let contract = ExternalWitContract::from_text("runtime.wit", CONSUMER_WIT, "consumer")
            .expect("consumer contract should resolve");
        let local_function = contract.restrict_binding_plan().imports[0]
            .local_function
            .clone();
        let source = format!("fun {local_function}: (value: Int32) -> Int32 = {{ value }}");
        let error = compile_external_wit_component(ExternalWitCompileRequest {
            source: &source,
            entry_path: Path::new("consumer.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &[],
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect_err("generated binding collisions must fail before type checking");
        assert!(matches!(
            error,
            ExternalWitCompileError::GeneratedBindingCollision(name)
                if name == local_function
        ));
    }

    #[test]
    fn generated_imports_cover_the_complete_scalar_foundation() {
        let contract = ExternalWitContract::from_text(
            "all-scalars.wit",
            r#"
                package external:all-scalars@1.0.0;
                interface values {
                    signed32: func(value: s32) -> s32;
                    signed64: func(value: s64) -> s64;
                    ratio: func(value: f64) -> f64;
                    flag: func(value: bool) -> bool;
                    marker: func(value: char) -> char;
                    notify: func();
                }
                world consumer {
                    import values;
                    export ready: func() -> bool;
                }
            "#,
            "consumer",
        )
        .expect("all scalar bindings should resolve");
        let implementations = [ExternalWitExportImplementation::new(
            contract.restrict_binding_plan().exports[0]
                .canonical_identity
                .clone(),
            "ready",
        )];
        let artifact = compile_external_wit_component(ExternalWitCompileRequest {
            source: "pub fun ready: () -> Boolean = { true }",
            entry_path: Path::new("consumer.rl"),
            package_roots: &[],
            contract: &contract,
            export_implementations: &implementations,
            optimization_level: WasmOptimizationLevel::None,
            arena_size_bytes: 0x1000,
        })
        .expect("all scalar import stubs should parse, type-check, and componentize");
        assert_eq!(artifact.binding_plan.imports.len(), 6);
        contract
            .validate_component(&artifact.component_wasm)
            .expect("all-scalar consumer should conform to external WIT");
    }
}
