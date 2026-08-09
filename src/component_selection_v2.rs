//! Manifest-neutral declarations for the synchronous Component ABI.
//!
//! Schema v2 records explicit resources, callable resources, and concrete
//! generic arguments without depending on Warder's TOML representation. This
//! module validates and normalizes the contract consumed by CIR, adapter,
//! code-generation, packaging, and runtime integration.

use std::collections::HashSet;
use thiserror::Error;

pub const COMPONENT_SELECTION_SCHEMA_V2: u32 = 2;
pub const COMPONENT_SELECTION_ABI_V2: &str = "canonical-v1-sync";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSelectionContract {
    pub schema_version: u32,
    pub package: String,
    pub world: String,
    pub abi_profile: String,
    pub exports: Vec<ComponentFunctionExportContract>,
    pub resources: Vec<ComponentResourceContract>,
    pub callables: Vec<ComponentCallableContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentFunctionExportContract {
    pub interface: String,
    pub source_function: String,
    pub external_name: String,
    /// Sorted by source type-parameter name. Declaration order is recovered
    /// and checked by the compiler's explicit-specialization pass.
    pub type_arguments: Vec<ComponentTypeArgumentContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentTypeArgumentContract {
    pub parameter: String,
    pub ty: ClosedComponentType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentResourceContract {
    pub interface: String,
    pub source_record: String,
    pub external_name: String,
    pub destructor: ComponentDestructorContract,
    pub methods: Vec<ComponentResourceMethodContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentDestructorContract {
    pub source_function: String,
    pub visibility: ComponentSourceVisibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentSourceVisibility {
    Private,
    Public,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentResourceMethodContract {
    pub source_function: String,
    pub external_name: String,
    pub receiver: ComponentResourceReceiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentResourceReceiver {
    Own,
    Borrow,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentCallableContract {
    pub interface: String,
    pub external_name: String,
    pub params: Vec<ComponentCallableParamContract>,
    pub result: ClosedComponentType,
    pub multiplicity: ComponentCallableMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentCallableParamContract {
    pub name: String,
    pub ty: ClosedComponentType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentCallableMultiplicity {
    Once,
    Many,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClosedComponentType {
    Unit,
    Int32,
    Int64,
    Float64,
    Boolean,
    String,
    Char,
    Named {
        name: String,
        arguments: Vec<ClosedComponentType>,
    },
    List(Box<ClosedComponentType>),
    Array {
        element: Box<ClosedComponentType>,
        length: u64,
    },
    Option(Box<ClosedComponentType>),
    Result {
        ok: Box<ClosedComponentType>,
        err: Box<ClosedComponentType>,
    },
    Function {
        params: Vec<ClosedComponentType>,
        result: Box<ClosedComponentType>,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ComponentSelectionContractError {
    #[error("unsupported component selection schema {0}; expected 1 or 2")]
    UnsupportedSchema(u32),
    #[error("unsupported component ABI profile '{0}'; expected 'canonical-v1-sync'")]
    UnsupportedAbi(String),
    #[error("invalid {path} '{value}': expected a lowercase kebab-case WIT identifier")]
    InvalidWitIdentifier { path: String, value: String },
    #[error("invalid {path} '{value}': expected one non-keyword Restrict identifier")]
    InvalidSourceIdentifier { path: String, value: String },
    #[error("component schema {schema} requires at least one explicit declaration")]
    EmptySelection { schema: u32 },
    #[error("component schema 1 does not accept {path}; use schema 2")]
    SchemaV1Extension { path: String },
    #[error("duplicate component external declaration '{interface}.{name}'")]
    DuplicateExternalName { interface: String, name: String },
    #[error("duplicate component function instance '{source_function}' with identical concrete type arguments")]
    DuplicateFunctionInstance { source_function: String },
    #[error("duplicate component resource source record '{0}'")]
    DuplicateResourceSource(String),
    #[error(
        "{path} destructor source '{source_function}' cannot also be declared as a resource method"
    )]
    DestructorAlsoMethod {
        path: String,
        source_function: String,
    },
    #[error("duplicate {path} method source '{source_function}'")]
    DuplicateResourceMethodSource {
        path: String,
        source_function: String,
    },
    #[error("duplicate {path} method name '{name}'")]
    DuplicateResourceMethodName { path: String, name: String },
    #[error("duplicate {path} callable parameter name '{name}'")]
    DuplicateCallableParam { path: String, name: String },
    #[error("callable '{interface}.{name}' uses multiplicity 'many'; synchronous schema v2 initially accepts only 'once'")]
    UnsupportedCallableMultiplicity { interface: String, name: String },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid closed type expression at byte {offset}: {message}")]
pub struct ClosedTypeExpressionError {
    pub offset: usize,
    pub message: String,
}

impl ComponentSelectionContract {
    pub fn try_new(
        schema_version: u32,
        package: impl Into<String>,
        world: impl Into<String>,
        abi_profile: impl Into<String>,
        mut exports: Vec<ComponentFunctionExportContract>,
        mut resources: Vec<ComponentResourceContract>,
        mut callables: Vec<ComponentCallableContract>,
    ) -> Result<Self, ComponentSelectionContractError> {
        if !matches!(schema_version, 1 | COMPONENT_SELECTION_SCHEMA_V2) {
            return Err(ComponentSelectionContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        let abi_profile = abi_profile.into();
        if abi_profile != COMPONENT_SELECTION_ABI_V2 {
            return Err(ComponentSelectionContractError::UnsupportedAbi(abi_profile));
        }
        let world = world.into();
        validate_wit_identifier("component.world", &world)?;

        for export in &mut exports {
            export
                .type_arguments
                .sort_by(|left, right| left.parameter.cmp(&right.parameter));
        }
        for resource in &mut resources {
            resource.methods.sort_by(|left, right| {
                (&left.external_name, &left.source_function, left.receiver).cmp(&(
                    &right.external_name,
                    &right.source_function,
                    right.receiver,
                ))
            });
        }
        exports.sort_by(|left, right| {
            (
                &left.interface,
                &left.external_name,
                &left.source_function,
                &left.type_arguments,
            )
                .cmp(&(
                    &right.interface,
                    &right.external_name,
                    &right.source_function,
                    &right.type_arguments,
                ))
        });
        resources.sort_by(|left, right| {
            (
                &left.interface,
                &left.external_name,
                &left.source_record,
                &left.destructor,
                &left.methods,
            )
                .cmp(&(
                    &right.interface,
                    &right.external_name,
                    &right.source_record,
                    &right.destructor,
                    &right.methods,
                ))
        });
        callables.sort_by(|left, right| {
            (
                &left.interface,
                &left.external_name,
                &left.params,
                &left.result,
                left.multiplicity,
            )
                .cmp(&(
                    &right.interface,
                    &right.external_name,
                    &right.params,
                    &right.result,
                    right.multiplicity,
                ))
        });

        if schema_version == 1 {
            if let Some((index, _)) = exports
                .iter()
                .enumerate()
                .find(|(_, export)| !export.type_arguments.is_empty())
            {
                return Err(ComponentSelectionContractError::SchemaV1Extension {
                    path: format!("component.exports[{index}].type-args"),
                });
            }
            if !resources.is_empty() {
                return Err(ComponentSelectionContractError::SchemaV1Extension {
                    path: "component.resources".to_string(),
                });
            }
            if !callables.is_empty() {
                return Err(ComponentSelectionContractError::SchemaV1Extension {
                    path: "component.callables".to_string(),
                });
            }
        }
        if exports.is_empty() && resources.is_empty() && callables.is_empty() {
            return Err(ComponentSelectionContractError::EmptySelection {
                schema: schema_version,
            });
        }

        let mut external_names = HashSet::new();
        let mut function_instances = HashSet::new();
        for (index, export) in exports.iter().enumerate() {
            let path = format!("component.exports[{index}]");
            validate_wit_identifier(&format!("{path}.interface"), &export.interface)?;
            validate_wit_identifier(&format!("{path}.name"), &export.external_name)?;
            validate_source_identifier(&format!("{path}.source"), &export.source_function)?;
            for argument in &export.type_arguments {
                validate_source_identifier(&format!("{path}.type-args key"), &argument.parameter)?;
            }
            insert_external_name(
                &mut external_names,
                &export.interface,
                &export.external_name,
            )?;
            let instance_key = (
                export.source_function.clone(),
                export.type_arguments.clone(),
            );
            if !function_instances.insert(instance_key) {
                return Err(ComponentSelectionContractError::DuplicateFunctionInstance {
                    source_function: export.source_function.clone(),
                });
            }
        }

        let mut resource_sources = HashSet::new();
        for (index, resource) in resources.iter().enumerate() {
            let path = format!("component.resources[{index}]");
            validate_wit_identifier(&format!("{path}.interface"), &resource.interface)?;
            validate_wit_identifier(&format!("{path}.name"), &resource.external_name)?;
            validate_source_identifier(&format!("{path}.source"), &resource.source_record)?;
            validate_source_identifier(
                &format!("{path}.destructor"),
                &resource.destructor.source_function,
            )?;
            insert_external_name(
                &mut external_names,
                &resource.interface,
                &resource.external_name,
            )?;
            if !resource_sources.insert(resource.source_record.clone()) {
                return Err(ComponentSelectionContractError::DuplicateResourceSource(
                    resource.source_record.clone(),
                ));
            }
            let mut method_sources = HashSet::new();
            let mut method_names = HashSet::new();
            for method in &resource.methods {
                validate_source_identifier(
                    &format!("{path}.methods.source"),
                    &method.source_function,
                )?;
                validate_wit_identifier(&format!("{path}.methods.name"), &method.external_name)?;
                if method.source_function == resource.destructor.source_function {
                    return Err(ComponentSelectionContractError::DestructorAlsoMethod {
                        path: path.clone(),
                        source_function: method.source_function.clone(),
                    });
                }
                if !method_sources.insert(method.source_function.clone()) {
                    return Err(
                        ComponentSelectionContractError::DuplicateResourceMethodSource {
                            path: path.clone(),
                            source_function: method.source_function.clone(),
                        },
                    );
                }
                if !method_names.insert(method.external_name.clone()) {
                    return Err(
                        ComponentSelectionContractError::DuplicateResourceMethodName {
                            path: path.clone(),
                            name: method.external_name.clone(),
                        },
                    );
                }
            }
        }

        for (index, callable) in callables.iter().enumerate() {
            let path = format!("component.callables[{index}]");
            validate_wit_identifier(&format!("{path}.interface"), &callable.interface)?;
            validate_wit_identifier(&format!("{path}.name"), &callable.external_name)?;
            insert_external_name(
                &mut external_names,
                &callable.interface,
                &callable.external_name,
            )?;
            if callable.multiplicity == ComponentCallableMultiplicity::Many {
                return Err(
                    ComponentSelectionContractError::UnsupportedCallableMultiplicity {
                        interface: callable.interface.clone(),
                        name: callable.external_name.clone(),
                    },
                );
            }
            let mut param_names = HashSet::new();
            for param in &callable.params {
                validate_wit_identifier(&format!("{path}.params.name"), &param.name)?;
                if !param_names.insert(param.name.clone()) {
                    return Err(ComponentSelectionContractError::DuplicateCallableParam {
                        path: path.clone(),
                        name: param.name.clone(),
                    });
                }
            }
        }

        Ok(Self {
            schema_version,
            package: package.into(),
            world,
            abi_profile,
            exports,
            resources,
            callables,
        })
    }
}

pub fn parse_closed_component_type(
    source: &str,
) -> Result<ClosedComponentType, ClosedTypeExpressionError> {
    let mut parser = TypeExpressionParser {
        source,
        offset: 0,
        depth: 0,
    };
    let ty = parser.parse_type()?;
    parser.skip_whitespace();
    if parser.offset != source.len() {
        return Err(parser.error("unexpected trailing input"));
    }
    Ok(ty)
}

fn insert_external_name(
    names: &mut HashSet<(String, String)>,
    interface: &str,
    name: &str,
) -> Result<(), ComponentSelectionContractError> {
    if !names.insert((interface.to_string(), name.to_string())) {
        return Err(ComponentSelectionContractError::DuplicateExternalName {
            interface: interface.to_string(),
            name: name.to_string(),
        });
    }
    Ok(())
}

fn validate_wit_identifier(path: &str, value: &str) -> Result<(), ComponentSelectionContractError> {
    if !crate::component::is_component_schema_v1_wit_identifier(value) {
        return Err(ComponentSelectionContractError::InvalidWitIdentifier {
            path: path.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn validate_source_identifier(
    path: &str,
    value: &str,
) -> Result<(), ComponentSelectionContractError> {
    if !crate::lexer::is_source_identifier(value) {
        return Err(ComponentSelectionContractError::InvalidSourceIdentifier {
            path: path.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

struct TypeExpressionParser<'a> {
    source: &'a str,
    offset: usize,
    depth: usize,
}

impl TypeExpressionParser<'_> {
    fn parse_type(&mut self) -> Result<ClosedComponentType, ClosedTypeExpressionError> {
        self.depth += 1;
        if self.depth > 64 {
            return Err(self.error("type expression nesting exceeds 64 levels"));
        }
        let left = self.parse_type_atom()?;
        self.skip_whitespace();
        let result = if self.consume("->") {
            let result = self.parse_type()?;
            ClosedComponentType::Function {
                params: vec![left],
                result: Box::new(result),
            }
        } else {
            left
        };
        self.depth -= 1;
        Ok(result)
    }

    fn parse_type_atom(&mut self) -> Result<ClosedComponentType, ClosedTypeExpressionError> {
        self.skip_whitespace();
        if self.consume("(") {
            self.skip_whitespace();
            if self.consume(")") {
                self.skip_whitespace();
                if self.consume("->") {
                    return Ok(ClosedComponentType::Function {
                        params: Vec::new(),
                        result: Box::new(self.parse_type()?),
                    });
                }
                return Ok(ClosedComponentType::Unit);
            }
            let mut types = vec![self.parse_type()?];
            self.skip_whitespace();
            while self.consume(",") {
                types.push(self.parse_type()?);
                self.skip_whitespace();
            }
            self.expect(")")?;
            self.skip_whitespace();
            if self.consume("->") {
                return Ok(ClosedComponentType::Function {
                    params: types,
                    result: Box::new(self.parse_type()?),
                });
            }
            if types.len() == 1 {
                return Ok(types.pop().expect("one grouped type was parsed"));
            }
            return Err(self.error("a multi-type parameter list must be followed by '->'"));
        }

        if self.peek() == Some('~') {
            return Err(self.error("temporal types are not closed synchronous type expressions"));
        }
        let name = self.parse_identifier()?;
        match name.as_str() {
            "Self" => return Err(self.error("'Self' is not a closed concrete type")),
            "Int32" => return self.finish_scalar(ClosedComponentType::Int32),
            "Int64" => return self.finish_scalar(ClosedComponentType::Int64),
            "Float64" => return self.finish_scalar(ClosedComponentType::Float64),
            "Boolean" => return self.finish_scalar(ClosedComponentType::Boolean),
            "String" => return self.finish_scalar(ClosedComponentType::String),
            "Char" => return self.finish_scalar(ClosedComponentType::Char),
            _ => {}
        }

        self.skip_whitespace();
        if name == "Array" && self.consume("<") {
            let element = self.parse_type()?;
            self.skip_whitespace();
            self.expect(",")?;
            self.skip_whitespace();
            let length = self.parse_array_length()?;
            self.skip_whitespace();
            self.expect(">")?;
            return Ok(ClosedComponentType::Array {
                element: Box::new(element),
                length,
            });
        }

        let arguments = if self.consume("<") {
            let mut arguments = Vec::new();
            loop {
                arguments.push(self.parse_type()?);
                self.skip_whitespace();
                if self.consume(">") {
                    break;
                }
                self.expect(",")?;
            }
            arguments
        } else {
            Vec::new()
        };
        match (name.as_str(), arguments.as_slice()) {
            ("List", [element]) => Ok(ClosedComponentType::List(Box::new(element.clone()))),
            ("Option", [inner]) => Ok(ClosedComponentType::Option(Box::new(inner.clone()))),
            ("Result", [ok, err]) => Ok(ClosedComponentType::Result {
                ok: Box::new(ok.clone()),
                err: Box::new(err.clone()),
            }),
            ("List" | "Option" | "Result" | "Array", _) => Err(self.error(
                "List and Option require one type argument, Result requires two, and Array requires an element type plus integer length",
            )),
            _ => Ok(ClosedComponentType::Named { name, arguments }),
        }
    }

    fn finish_scalar(
        &mut self,
        scalar: ClosedComponentType,
    ) -> Result<ClosedComponentType, ClosedTypeExpressionError> {
        self.skip_whitespace();
        if self.peek() == Some('<') {
            return Err(self.error("scalar types do not accept type arguments"));
        }
        Ok(scalar)
    }

    fn parse_identifier(&mut self) -> Result<String, ClosedTypeExpressionError> {
        self.skip_whitespace();
        let start = self.offset;
        let Some(first) = self.peek() else {
            return Err(self.error("expected a type"));
        };
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return Err(self.error("expected a Restrict type identifier"));
        }
        self.offset += first.len_utf8();
        while let Some(character) = self.peek() {
            if character == '_' || character.is_ascii_alphanumeric() {
                self.offset += character.len_utf8();
            } else {
                break;
            }
        }
        let name = &self.source[start..self.offset];
        if name == "_" || !crate::lexer::is_source_identifier(name) {
            return Err(ClosedTypeExpressionError {
                offset: start,
                message: format!("'{name}' is not a concrete Restrict type identifier"),
            });
        }
        Ok(name.to_string())
    }

    fn parse_array_length(&mut self) -> Result<u64, ClosedTypeExpressionError> {
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.offset += 1;
        }
        if start == self.offset {
            return Err(self.error("expected an unsigned integer array length"));
        }
        self.source[start..self.offset]
            .parse()
            .map_err(|_| ClosedTypeExpressionError {
                offset: start,
                message: "array length exceeds the supported u64 range".to_string(),
            })
    }

    fn expect(&mut self, token: &str) -> Result<(), ClosedTypeExpressionError> {
        if self.consume(token) {
            Ok(())
        } else {
            Err(self.error(&format!("expected '{token}'")))
        }
    }

    fn consume(&mut self, token: &str) -> bool {
        self.skip_whitespace();
        if self.source[self.offset..].starts_with(token) {
            self.offset += token.len();
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.offset += self.peek().expect("peeked whitespace").len_utf8();
        }
    }

    fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    fn error(&self, message: &str) -> ClosedTypeExpressionError {
        ClosedTypeExpressionError {
            offset: self.offset,
            message: message.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export(source: &str, name: &str) -> ComponentFunctionExportContract {
        ComponentFunctionExportContract {
            interface: "api".to_string(),
            source_function: source.to_string(),
            external_name: name.to_string(),
            type_arguments: Vec::new(),
        }
    }

    #[test]
    fn parses_recursive_closed_types_and_callable_signatures() {
        assert_eq!(
            parse_closed_component_type("Result<Array<String, 4>, List<Failure>>").unwrap(),
            ClosedComponentType::Result {
                ok: Box::new(ClosedComponentType::Array {
                    element: Box::new(ClosedComponentType::String),
                    length: 4,
                }),
                err: Box::new(ClosedComponentType::List(Box::new(
                    ClosedComponentType::Named {
                        name: "Failure".to_string(),
                        arguments: Vec::new(),
                    }
                ))),
            }
        );
        assert!(matches!(
            parse_closed_component_type("(String, Int32) -> Option<String>").unwrap(),
            ClosedComponentType::Function { params, .. } if params.len() == 2
        ));
    }

    #[test]
    fn rejects_temporal_open_and_malformed_type_expressions() {
        for source in [
            "File<~t>",
            "_",
            "List<> ",
            "Array<String, N>",
            "Int32<String>",
        ] {
            assert!(
                parse_closed_component_type(source).is_err(),
                "{source} should be rejected"
            );
        }
    }

    #[test]
    fn contract_normalizes_order_and_rejects_duplicate_instances() {
        let first = ComponentSelectionContract::try_new(
            COMPONENT_SELECTION_SCHEMA_V2,
            "example:pkg@0.1.0",
            "test-world",
            COMPONENT_SELECTION_ABI_V2,
            vec![export("zeta", "zeta"), export("alpha", "alpha")],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let second = ComponentSelectionContract::try_new(
            COMPONENT_SELECTION_SCHEMA_V2,
            "example:pkg@0.1.0",
            "test-world",
            COMPONENT_SELECTION_ABI_V2,
            vec![export("alpha", "alpha"), export("zeta", "zeta")],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(first, second);

        assert!(matches!(
            ComponentSelectionContract::try_new(
                COMPONENT_SELECTION_SCHEMA_V2,
                "example:pkg@0.1.0",
                "test-world",
                COMPONENT_SELECTION_ABI_V2,
                vec![export("alpha", "first"), export("alpha", "second")],
                Vec::new(),
                Vec::new(),
            ),
            Err(ComponentSelectionContractError::DuplicateFunctionInstance { .. })
        ));
    }

    #[test]
    fn schema_v1_rejects_v2_facts_and_many_is_typed_unsupported() {
        let mut generic = export("identity", "identity-string");
        generic.type_arguments.push(ComponentTypeArgumentContract {
            parameter: "T".to_string(),
            ty: ClosedComponentType::String,
        });
        assert!(matches!(
            ComponentSelectionContract::try_new(
                1,
                "example:pkg@0.1.0",
                "test-world",
                COMPONENT_SELECTION_ABI_V2,
                vec![generic],
                Vec::new(),
                Vec::new(),
            ),
            Err(ComponentSelectionContractError::SchemaV1Extension { .. })
        ));

        let callable = ComponentCallableContract {
            interface: "api".to_string(),
            external_name: "callback".to_string(),
            params: Vec::new(),
            result: ClosedComponentType::Unit,
            multiplicity: ComponentCallableMultiplicity::Many,
        };
        assert!(matches!(
            ComponentSelectionContract::try_new(
                COMPONENT_SELECTION_SCHEMA_V2,
                "example:pkg@0.1.0",
                "test-world",
                COMPONENT_SELECTION_ABI_V2,
                Vec::new(),
                Vec::new(),
                vec![callable],
            ),
            Err(ComponentSelectionContractError::UnsupportedCallableMultiplicity { .. })
        ));
    }
}
