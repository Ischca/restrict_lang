//! Constraint primitives for A-layer type inference.
//!
//! This module defines inference variables, constraints, substitution,
//! unification, and finalization for the A-layer solver. The affine-aware type
//! checker can feed ordinary equality constraints into this layer while keeping
//! source evaluation order in the B-layer.

use crate::type_checker::{format_typed_type, ArrayLength, TypeError, TypedType};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarId(pub u32);

impl fmt::Display for TypeVarId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.0)
    }
}

#[derive(Debug, Default)]
pub struct TypeVarGenerator {
    next: u32,
}

impl TypeVarGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fresh_id(&mut self) -> TypeVarId {
        let id = TypeVarId(self.next);
        self.next += 1;
        id
    }

    pub fn fresh_var(&mut self) -> TypedType {
        TypedType::InferVar(self.fresh_id())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    TypeEquals {
        expected: TypedType,
        actual: TypedType,
        origin: ConstraintOrigin,
    },
    HasForm {
        ty: TypedType,
        form_name: String,
        origin: ConstraintOrigin,
    },
    AssociatedTypeResolution {
        base_type: TypedType,
        form_name: String,
        assoc_name: String,
        type_args: Vec<TypedType>,
        result: TypedType,
        origin: ConstraintOrigin,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintOrigin {
    pub span: Option<Span>,
    pub kind: ConstraintKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintKind {
    Argument { func_name: String, arg_index: usize },
    ReturnAnnotation { var_name: String },
    LambdaParam { param_name: String },
    LambdaReturn,
    FormBound { type_param: String },
    AssocTypeProjection { assoc_name: String },
    Apply,
}

#[derive(Debug, Clone, Default)]
pub struct Substitution {
    bindings: HashMap<TypeVarId, TypedType>,
}

impl Substitution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, id: TypeVarId, ty: TypedType) {
        self.bindings.insert(id, ty);
    }

    pub fn get(&self, id: TypeVarId) -> Option<&TypedType> {
        self.bindings.get(&id)
    }

    pub fn apply(&self, ty: &TypedType) -> Result<TypedType, TypeError> {
        zonk(ty, self)
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }
}

pub fn fresh_type_param_map(
    type_param_names: &[String],
    vars: &mut TypeVarGenerator,
) -> HashMap<String, TypedType> {
    type_param_names
        .iter()
        .map(|name| (name.clone(), vars.fresh_var()))
        .collect()
}

pub fn substitute_type_params(ty: &TypedType, type_vars: &HashMap<String, TypedType>) -> TypedType {
    match ty {
        TypedType::TypeParam(name) => type_vars.get(name).cloned().unwrap_or_else(|| ty.clone()),
        TypedType::List(inner) => {
            TypedType::List(Box::new(substitute_type_params(inner, type_vars)))
        }
        TypedType::Option(inner) => {
            TypedType::Option(Box::new(substitute_type_params(inner, type_vars)))
        }
        TypedType::Result(ok, err) => TypedType::Result(
            Box::new(substitute_type_params(ok, type_vars)),
            Box::new(substitute_type_params(err, type_vars)),
        ),
        TypedType::Array(inner, size) => {
            TypedType::Array(Box::new(substitute_type_params(inner, type_vars)), *size)
        }
        TypedType::Function {
            params,
            return_type,
        } => TypedType::Function {
            params: params
                .iter()
                .map(|param| substitute_type_params(param, type_vars))
                .collect(),
            return_type: Box::new(substitute_type_params(return_type, type_vars)),
        },
        TypedType::Record {
            name,
            type_args,
            frozen,
            hash,
            parent_hash,
        } => TypedType::Record {
            name: name.clone(),
            type_args: type_args
                .iter()
                .map(|arg| substitute_type_params(arg, type_vars))
                .collect(),
            frozen: *frozen,
            hash: hash.clone(),
            parent_hash: parent_hash.clone(),
        },
        TypedType::Temporal {
            base_type,
            temporals,
        } => TypedType::Temporal {
            base_type: Box::new(substitute_type_params(base_type, type_vars)),
            temporals: temporals.clone(),
        },
        TypedType::Projection {
            base,
            form_name,
            assoc_name,
            args,
        } => TypedType::Projection {
            base: Box::new(substitute_type_params(base, type_vars)),
            form_name: form_name.clone(),
            assoc_name: assoc_name.clone(),
            args: args
                .iter()
                .map(|arg| substitute_type_params(arg, type_vars))
                .collect(),
        },
        _ => ty.clone(),
    }
}

#[derive(Debug, Clone, Default)]
pub struct FormEnvironment {
    adoptions: Vec<FormAdoption>,
}

#[derive(Debug, Clone)]
struct FormAdoption {
    form_name: String,
    type_constructor: TypeConstructor,
    associated_types: Vec<AssociatedTypeImplementation>,
}

#[derive(Debug, Clone, Copy)]
struct AssociatedTypeImplementation {
    assoc_name: &'static str,
    resolver: AssociatedTypeResolver,
}

#[derive(Debug, Clone, Copy)]
enum AssociatedTypeResolver {
    Item,
    Mapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TypeConstructor {
    List,
    Option,
}

const LIST_CONTAINER_ASSOCIATED_TYPES: &[AssociatedTypeImplementation] = &[
    AssociatedTypeImplementation {
        assoc_name: "Item",
        resolver: AssociatedTypeResolver::Item,
    },
    AssociatedTypeImplementation {
        assoc_name: "Mapped",
        resolver: AssociatedTypeResolver::Mapped,
    },
];

const OPTION_CONTAINER_ASSOCIATED_TYPES: &[AssociatedTypeImplementation] = &[
    AssociatedTypeImplementation {
        assoc_name: "Item",
        resolver: AssociatedTypeResolver::Item,
    },
    AssociatedTypeImplementation {
        assoc_name: "Value",
        resolver: AssociatedTypeResolver::Item,
    },
    AssociatedTypeImplementation {
        assoc_name: "Mapped",
        resolver: AssociatedTypeResolver::Mapped,
    },
];

impl FormEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn standard() -> Self {
        let mut env = Self::new();
        env.register_builtin_container_adoptions()
            .expect("standard Container adoptions should be valid");
        env
    }

    pub(crate) fn register_builtin_container_adoptions(&mut self) -> Result<(), TypeError> {
        self.adopt_type_constructor(
            TypeConstructor::List,
            "Container",
            LIST_CONTAINER_ASSOCIATED_TYPES,
        )?;
        self.adopt_type_constructor(
            TypeConstructor::Option,
            "Container",
            OPTION_CONTAINER_ASSOCIATED_TYPES,
        )?;
        Ok(())
    }

    fn adopt_type_constructor(
        &mut self,
        type_constructor: TypeConstructor,
        form_name: &str,
        associated_types: &[AssociatedTypeImplementation],
    ) -> Result<(), TypeError> {
        if self.adoptions.iter().any(|adoption| {
            adoption.form_name == form_name && adoption.type_constructor == type_constructor
        }) {
            return Err(TypeError::UnsupportedFeature(format!(
                "duplicate built-in {} adoption for {}",
                form_name,
                type_constructor.name()
            )));
        }

        let mut associated_type_names = std::collections::HashSet::new();
        for associated_type in associated_types {
            if !associated_type_names.insert(associated_type.assoc_name) {
                return Err(TypeError::UnsupportedFeature(format!(
                    "duplicate associated type {} in built-in {} adoption for {}",
                    associated_type.assoc_name,
                    form_name,
                    type_constructor.name()
                )));
            }
        }

        self.adoptions.push(FormAdoption {
            form_name: form_name.to_string(),
            type_constructor,
            associated_types: associated_types.to_vec(),
        });

        Ok(())
    }

    fn require_form(&self, ty: &TypedType, form_name: &str) -> Result<(), TypeError> {
        self.find_adoption(ty, form_name).map(|_| ())
    }

    fn resolve_associated_type(
        &self,
        base_type: &TypedType,
        form_name: &str,
        assoc_name: &str,
        type_args: &[TypedType],
    ) -> Result<TypedType, TypeError> {
        let adoption = self.find_adoption(base_type, form_name)?;
        let associated_type = adoption
            .associated_types
            .iter()
            .find(|associated_type| associated_type.assoc_name == assoc_name)
            .ok_or_else(|| unresolved_projection_error(base_type, form_name, assoc_name))?;

        associated_type.resolve(adoption.type_constructor, base_type, type_args)
    }

    fn find_adoption(&self, ty: &TypedType, form_name: &str) -> Result<&FormAdoption, TypeError> {
        let constructor =
            type_constructor_of(ty).ok_or_else(|| unsupported_form_error(ty, form_name))?;

        self.adoptions
            .iter()
            .find(|adoption| {
                adoption.form_name == form_name && adoption.type_constructor == constructor
            })
            .ok_or_else(|| unsupported_form_error(ty, form_name))
    }
}

impl AssociatedTypeImplementation {
    fn resolve(
        self,
        type_constructor: TypeConstructor,
        base_type: &TypedType,
        type_args: &[TypedType],
    ) -> Result<TypedType, TypeError> {
        match self.resolver {
            AssociatedTypeResolver::Item => {
                ensure_no_type_args(self.assoc_name, type_args)?;
                Ok(type_constructor.item_type(base_type))
            }
            AssociatedTypeResolver::Mapped => {
                let mapped_item = single_type_arg(self.assoc_name, type_args)?;
                Ok(type_constructor.with_item_type(mapped_item.clone()))
            }
        }
    }
}

impl TypeConstructor {
    fn name(self) -> &'static str {
        match self {
            TypeConstructor::List => "List",
            TypeConstructor::Option => "Option",
        }
    }

    fn item_type(self, ty: &TypedType) -> TypedType {
        match (self, ty) {
            (TypeConstructor::List, TypedType::List(item))
            | (TypeConstructor::Option, TypedType::Option(item)) => (**item).clone(),
            _ => unreachable!("type constructor must match adopted type"),
        }
    }

    fn with_item_type(self, item: TypedType) -> TypedType {
        match self {
            TypeConstructor::List => TypedType::List(Box::new(item)),
            TypeConstructor::Option => TypedType::Option(Box::new(item)),
        }
    }
}

pub fn solve_constraints(constraints: &[Constraint]) -> Result<Substitution, TypeError> {
    solve_constraints_with_forms(constraints, &FormEnvironment::standard())
}

pub fn solve_constraints_with_initial(
    constraints: &[Constraint],
    initial: &Substitution,
) -> Result<Substitution, TypeError> {
    solve_constraints_with_forms_and_initial(constraints, &FormEnvironment::standard(), initial)
}

pub fn solve_constraints_partial_with_initial(
    constraints: &[Constraint],
    initial: &Substitution,
) -> Result<Substitution, TypeError> {
    solve_constraints_partial_with_forms_and_initial(
        constraints,
        &FormEnvironment::standard(),
        initial,
    )
}

pub fn solve_constraints_with_forms(
    constraints: &[Constraint],
    forms: &FormEnvironment,
) -> Result<Substitution, TypeError> {
    solve_constraints_with_forms_and_initial(constraints, forms, &Substitution::new())
}

pub fn solve_constraints_with_forms_and_initial(
    constraints: &[Constraint],
    forms: &FormEnvironment,
    initial: &Substitution,
) -> Result<Substitution, TypeError> {
    solve_constraints_with_forms_and_initial_mode(
        constraints,
        forms,
        initial,
        SolverMode::RequireComplete,
    )
}

pub fn solve_constraints_partial_with_forms_and_initial(
    constraints: &[Constraint],
    forms: &FormEnvironment,
    initial: &Substitution,
) -> Result<Substitution, TypeError> {
    solve_constraints_with_forms_and_initial_mode(
        constraints,
        forms,
        initial,
        SolverMode::AllowDeferred,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SolverMode {
    RequireComplete,
    AllowDeferred,
}

fn solve_constraints_with_forms_and_initial_mode(
    constraints: &[Constraint],
    forms: &FormEnvironment,
    initial: &Substitution,
    mode: SolverMode,
) -> Result<Substitution, TypeError> {
    let mut substitution = initial.clone();
    let mut worklist = constraints.to_vec();

    while !worklist.is_empty() {
        let mut changed = false;
        let mut deferred = Vec::new();

        for constraint in worklist {
            let constraint = apply_constraint(&constraint, &substitution)?;
            let bindings_before = substitution.len();

            match constraint {
                Constraint::TypeEquals {
                    expected,
                    actual,
                    origin,
                } => {
                    if contains_projection(&expected) || contains_projection(&actual) {
                        deferred.push(Constraint::TypeEquals {
                            expected,
                            actual,
                            origin,
                        });
                        continue;
                    }

                    unify(&expected, &actual, &mut substitution)
                        .map_err(|err| with_constraint_origin(err, &origin))?;
                }
                Constraint::HasForm {
                    ty,
                    form_name,
                    origin,
                } => {
                    if should_defer_form_target(&ty) {
                        deferred.push(Constraint::HasForm {
                            ty,
                            form_name,
                            origin,
                        });
                        continue;
                    }

                    forms
                        .require_form(&ty, &form_name)
                        .map_err(|err| with_constraint_origin(err, &origin))?;
                }
                Constraint::AssociatedTypeResolution {
                    base_type,
                    form_name,
                    assoc_name,
                    type_args,
                    result,
                    origin,
                } => {
                    if should_defer_form_target(&base_type)
                        || type_args.iter().any(contains_projection)
                    {
                        deferred.push(Constraint::AssociatedTypeResolution {
                            base_type,
                            form_name,
                            assoc_name,
                            type_args,
                            result,
                            origin,
                        });
                        continue;
                    }

                    let resolved = forms
                        .resolve_associated_type(&base_type, &form_name, &assoc_name, &type_args)
                        .map_err(|err| with_constraint_origin(err, &origin))?;
                    unify(&result, &resolved, &mut substitution)
                        .map_err(|err| with_constraint_origin(err, &origin))?;
                }
            }

            changed |= substitution.len() != bindings_before;
        }

        if deferred.is_empty() {
            return Ok(substitution);
        }

        if !changed {
            if mode == SolverMode::AllowDeferred {
                return Ok(substitution);
            }
            return Err(unresolved_constraint_error(&deferred[0]));
        }

        worklist = deferred;
    }

    Ok(substitution)
}

fn unsupported_form_error(ty: &TypedType, form_name: &str) -> TypeError {
    TypeError::UnsupportedFeature(format!(
        "{} does not satisfy the built-in {} constraint",
        format_typed_type(ty),
        form_name
    ))
}

fn unresolved_projection_error(
    base_type: &TypedType,
    form_name: &str,
    assoc_name: &str,
) -> TypeError {
    TypeError::UnresolvedProjection(format!(
        "{} as {}.{}",
        format_typed_type(base_type),
        form_name,
        assoc_name
    ))
}

fn type_constructor_of(ty: &TypedType) -> Option<TypeConstructor> {
    match ty {
        TypedType::List(_) => Some(TypeConstructor::List),
        TypedType::Option(_) => Some(TypeConstructor::Option),
        _ => None,
    }
}

fn should_defer_form_target(ty: &TypedType) -> bool {
    contains_projection(ty) || (type_constructor_of(ty).is_none() && contains_infer_var(ty))
}

fn ensure_no_type_args(assoc_name: &str, type_args: &[TypedType]) -> Result<(), TypeError> {
    if type_args.is_empty() {
        Ok(())
    } else {
        Err(TypeError::UnresolvedProjection(format!(
            "{} expects no type arguments, found {}",
            assoc_name,
            type_args.len()
        )))
    }
}

fn single_type_arg<'a>(
    assoc_name: &str,
    type_args: &'a [TypedType],
) -> Result<&'a TypedType, TypeError> {
    match type_args {
        [arg] => Ok(arg),
        _ => Err(TypeError::UnresolvedProjection(format!(
            "{} expects one type argument, found {}",
            assoc_name,
            type_args.len()
        ))),
    }
}

fn unresolved_constraint_error(constraint: &Constraint) -> TypeError {
    match constraint {
        Constraint::HasForm {
            ty,
            form_name,
            origin,
        } => with_constraint_origin(
            TypeError::UnsupportedFeature(format!(
                "could not prove {} satisfies the built-in {} constraint",
                format_typed_type(ty),
                form_name
            )),
            origin,
        ),
        Constraint::AssociatedTypeResolution {
            base_type,
            form_name,
            assoc_name,
            type_args,
            origin,
            ..
        } => {
            let args = type_args
                .iter()
                .map(format_typed_type)
                .collect::<Vec<_>>()
                .join(", ");
            with_constraint_origin(
                TypeError::UnresolvedProjection(format!(
                    "{} as {}.{}<{}>",
                    format_typed_type(base_type),
                    form_name,
                    assoc_name,
                    args
                )),
                origin,
            )
        }
        Constraint::TypeEquals {
            expected,
            actual,
            origin,
        } => with_constraint_origin(
            TypeError::UnresolvedProjection(format!(
                "{} = {}",
                format_typed_type(expected),
                format_typed_type(actual)
            )),
            origin,
        ),
    }
}

fn with_constraint_origin(error: TypeError, origin: &ConstraintOrigin) -> TypeError {
    let Some(context) = constraint_origin_context(origin) else {
        return error;
    };

    match error {
        TypeError::TypeMismatch { expected, found } => TypeError::TypeMismatch {
            expected,
            found: append_context(found, &context),
        },
        TypeError::CannotInferType(message) => {
            TypeError::CannotInferType(append_context(message, &context))
        }
        TypeError::UnresolvedProjection(message) => {
            TypeError::UnresolvedProjection(append_context(message, &context))
        }
        TypeError::UnsupportedFeature(message) => {
            TypeError::UnsupportedFeature(append_context(message, &context))
        }
        other => other,
    }
}

fn constraint_origin_context(origin: &ConstraintOrigin) -> Option<String> {
    match &origin.kind {
        ConstraintKind::Argument {
            func_name,
            arg_index,
        } => Some(format!("argument {} of {}", arg_index + 1, func_name)),
        ConstraintKind::ReturnAnnotation { var_name } => {
            Some(format!("return annotation of {}", var_name))
        }
        ConstraintKind::LambdaParam { param_name } => {
            Some(format!("lambda parameter {}", param_name))
        }
        ConstraintKind::LambdaReturn => Some("lambda return".to_string()),
        ConstraintKind::FormBound { type_param } => Some(format!("form bound of {}", type_param)),
        ConstraintKind::AssocTypeProjection { assoc_name } => {
            Some(format!("associated type projection {}", assoc_name))
        }
        ConstraintKind::Apply => None,
    }
}

fn append_context(message: String, context: &str) -> String {
    format!("{} ({})", message, context)
}

fn array_lengths_unify(left: ArrayLength, right: ArrayLength) -> bool {
    match (left, right) {
        (ArrayLength::AnyInternal, _) | (_, ArrayLength::AnyInternal) => true,
        (ArrayLength::Known(left), ArrayLength::Known(right)) => left == right,
    }
}

pub fn unify(
    expected: &TypedType,
    actual: &TypedType,
    substitution: &mut Substitution,
) -> Result<(), TypeError> {
    let expected = zonk(expected, substitution)?;
    let actual = zonk(actual, substitution)?;

    match (&expected, &actual) {
        (TypedType::InferVar(left), TypedType::InferVar(right)) if left == right => Ok(()),
        (TypedType::InferVar(id), ty) | (ty, TypedType::InferVar(id)) => {
            bind_infer_var(*id, ty, substitution)
        }
        (TypedType::Int32, TypedType::Int32)
        | (TypedType::Int64, TypedType::Int64)
        | (TypedType::Float64, TypedType::Float64)
        | (TypedType::Boolean, TypedType::Boolean)
        | (TypedType::String, TypedType::String)
        | (TypedType::Char, TypedType::Char)
        | (TypedType::Unit, TypedType::Unit) => Ok(()),
        (TypedType::TypeParam(left), TypedType::TypeParam(right)) if left == right => Ok(()),
        (TypedType::List(left), TypedType::List(right))
        | (TypedType::Option(left), TypedType::Option(right)) => unify(left, right, substitution),
        (TypedType::Result(left_ok, left_err), TypedType::Result(right_ok, right_err)) => {
            unify(left_ok, right_ok, substitution)?;
            unify(left_err, right_err, substitution)
        }
        (TypedType::Array(left_ty, left_size), TypedType::Array(right_ty, right_size)) => {
            if !array_lengths_unify(*left_size, *right_size) {
                return type_mismatch(&expected, &actual);
            }
            unify(left_ty, right_ty, substitution)
        }
        (
            TypedType::Function {
                params: left_params,
                return_type: left_return,
            },
            TypedType::Function {
                params: right_params,
                return_type: right_return,
            },
        ) => {
            if left_params.len() != right_params.len() {
                return type_mismatch(&expected, &actual);
            }
            for (left, right) in left_params.iter().zip(right_params.iter()) {
                unify(left, right, substitution)?;
            }
            unify(left_return, right_return, substitution)
        }
        (
            TypedType::Record {
                name: left_name,
                type_args: left_type_args,
                frozen: left_frozen,
                hash: left_hash,
                parent_hash: left_parent_hash,
            },
            TypedType::Record {
                name: right_name,
                type_args: right_type_args,
                frozen: right_frozen,
                hash: right_hash,
                parent_hash: right_parent_hash,
            },
        ) if left_name == right_name
            && left_type_args.len() == right_type_args.len()
            && left_frozen == right_frozen
            && left_hash == right_hash
            && left_parent_hash == right_parent_hash =>
        {
            for (left_arg, right_arg) in left_type_args.iter().zip(right_type_args.iter()) {
                unify(left_arg, right_arg, substitution)?;
            }
            Ok(())
        }
        (
            TypedType::Temporal {
                base_type: left,
                temporals: left_temporals,
            },
            TypedType::Temporal {
                base_type: right,
                temporals: right_temporals,
            },
        ) => {
            if left_temporals != right_temporals {
                return type_mismatch(&expected, &actual);
            }
            unify(left, right, substitution)
        }
        (TypedType::Projection { .. }, _) | (_, TypedType::Projection { .. }) => {
            Err(TypeError::UnresolvedProjection(format!(
                "{} = {}",
                format_typed_type(&expected),
                format_typed_type(&actual)
            )))
        }
        _ => type_mismatch(&expected, &actual),
    }
}

pub fn zonk(ty: &TypedType, substitution: &Substitution) -> Result<TypedType, TypeError> {
    match ty {
        TypedType::InferVar(id) => {
            if let Some(bound) = substitution.get(*id) {
                zonk(bound, substitution)
            } else {
                Ok(ty.clone())
            }
        }
        TypedType::List(inner) => Ok(TypedType::List(Box::new(zonk(inner, substitution)?))),
        TypedType::Option(inner) => Ok(TypedType::Option(Box::new(zonk(inner, substitution)?))),
        TypedType::Result(ok, err) => Ok(TypedType::Result(
            Box::new(zonk(ok, substitution)?),
            Box::new(zonk(err, substitution)?),
        )),
        TypedType::Array(inner, size) => Ok(TypedType::Array(
            Box::new(zonk(inner, substitution)?),
            *size,
        )),
        TypedType::Function {
            params,
            return_type,
        } => Ok(TypedType::Function {
            params: params
                .iter()
                .map(|param| zonk(param, substitution))
                .collect::<Result<Vec<_>, _>>()?,
            return_type: Box::new(zonk(return_type, substitution)?),
        }),
        TypedType::Record {
            name,
            type_args,
            frozen,
            hash,
            parent_hash,
        } => Ok(TypedType::Record {
            name: name.clone(),
            type_args: type_args
                .iter()
                .map(|arg| zonk(arg, substitution))
                .collect::<Result<Vec<_>, _>>()?,
            frozen: *frozen,
            hash: hash.clone(),
            parent_hash: parent_hash.clone(),
        }),
        TypedType::Temporal {
            base_type,
            temporals,
        } => Ok(TypedType::Temporal {
            base_type: Box::new(zonk(base_type, substitution)?),
            temporals: temporals.clone(),
        }),
        TypedType::Projection {
            base,
            form_name,
            assoc_name,
            args,
        } => Ok(TypedType::Projection {
            base: Box::new(zonk(base, substitution)?),
            form_name: form_name.clone(),
            assoc_name: assoc_name.clone(),
            args: args
                .iter()
                .map(|arg| zonk(arg, substitution))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        _ => Ok(ty.clone()),
    }
}

pub fn finalize_type(ty: &TypedType, substitution: &Substitution) -> Result<TypedType, TypeError> {
    let zonked = zonk(ty, substitution)?;
    if contains_infer_var(&zonked) {
        return Err(TypeError::CannotInferType(format_typed_type(&zonked)));
    }
    if contains_projection(&zonked) {
        return Err(TypeError::UnresolvedProjection(format_typed_type(&zonked)));
    }
    Ok(zonked)
}

pub fn contains_infer_var(ty: &TypedType) -> bool {
    match ty {
        TypedType::InferVar(_) => true,
        TypedType::List(inner) | TypedType::Option(inner) | TypedType::Array(inner, _) => {
            contains_infer_var(inner)
        }
        TypedType::Result(ok, err) => contains_infer_var(ok) || contains_infer_var(err),
        TypedType::Function {
            params,
            return_type,
        } => params.iter().any(contains_infer_var) || contains_infer_var(return_type),
        TypedType::Record { type_args, .. } => type_args.iter().any(contains_infer_var),
        TypedType::Temporal { base_type, .. } => contains_infer_var(base_type),
        TypedType::Projection { base, args, .. } => {
            contains_infer_var(base) || args.iter().any(contains_infer_var)
        }
        _ => false,
    }
}

pub fn contains_projection(ty: &TypedType) -> bool {
    match ty {
        TypedType::Projection { .. } => true,
        TypedType::List(inner) | TypedType::Option(inner) | TypedType::Array(inner, _) => {
            contains_projection(inner)
        }
        TypedType::Result(ok, err) => contains_projection(ok) || contains_projection(err),
        TypedType::Function {
            params,
            return_type,
        } => params.iter().any(contains_projection) || contains_projection(return_type),
        TypedType::Record { type_args, .. } => type_args.iter().any(contains_projection),
        TypedType::Temporal { base_type, .. } => contains_projection(base_type),
        _ => false,
    }
}

fn bind_infer_var(
    id: TypeVarId,
    ty: &TypedType,
    substitution: &mut Substitution,
) -> Result<(), TypeError> {
    if occurs_in(id, ty, substitution)? {
        return Err(TypeError::CannotInferType(format!(
            "recursive type involving {}",
            id
        )));
    }

    substitution.bind(id, ty.clone());
    Ok(())
}

fn occurs_in(
    id: TypeVarId,
    ty: &TypedType,
    substitution: &Substitution,
) -> Result<bool, TypeError> {
    let zonked = zonk(ty, substitution)?;
    Ok(match &zonked {
        TypedType::InferVar(other) => id == *other,
        TypedType::List(inner) | TypedType::Option(inner) | TypedType::Array(inner, _) => {
            occurs_in(id, inner, substitution)?
        }
        TypedType::Result(ok, err) => {
            occurs_in(id, ok, substitution)? || occurs_in(id, err, substitution)?
        }
        TypedType::Function {
            params,
            return_type,
        } => {
            params
                .iter()
                .map(|param| occurs_in(id, param, substitution))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .any(|found| found)
                || occurs_in(id, return_type, substitution)?
        }
        TypedType::Record { type_args, .. } => type_args
            .iter()
            .map(|arg| occurs_in(id, arg, substitution))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .any(|found| found),
        TypedType::Temporal { base_type, .. } => occurs_in(id, base_type, substitution)?,
        TypedType::Projection { base, args, .. } => {
            occurs_in(id, base, substitution)?
                || args
                    .iter()
                    .map(|arg| occurs_in(id, arg, substitution))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .any(|found| found)
        }
        _ => false,
    })
}

fn apply_constraint(
    constraint: &Constraint,
    substitution: &Substitution,
) -> Result<Constraint, TypeError> {
    match constraint {
        Constraint::TypeEquals {
            expected,
            actual,
            origin,
        } => Ok(Constraint::TypeEquals {
            expected: zonk(expected, substitution)?,
            actual: zonk(actual, substitution)?,
            origin: origin.clone(),
        }),
        Constraint::HasForm {
            ty,
            form_name,
            origin,
        } => Ok(Constraint::HasForm {
            ty: zonk(ty, substitution)?,
            form_name: form_name.clone(),
            origin: origin.clone(),
        }),
        Constraint::AssociatedTypeResolution {
            base_type,
            form_name,
            assoc_name,
            type_args,
            result,
            origin,
        } => Ok(Constraint::AssociatedTypeResolution {
            base_type: zonk(base_type, substitution)?,
            form_name: form_name.clone(),
            assoc_name: assoc_name.clone(),
            type_args: type_args
                .iter()
                .map(|arg| zonk(arg, substitution))
                .collect::<Result<Vec<_>, _>>()?,
            result: zonk(result, substitution)?,
            origin: origin.clone(),
        }),
    }
}

fn type_mismatch(expected: &TypedType, actual: &TypedType) -> Result<(), TypeError> {
    Err(TypeError::TypeMismatch {
        expected: format_typed_type(expected),
        found: format_typed_type(actual),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_environment_rejects_duplicate_builtin_adoption() {
        let mut env = FormEnvironment::new();
        env.adopt_type_constructor(
            TypeConstructor::List,
            "Container",
            LIST_CONTAINER_ASSOCIATED_TYPES,
        )
        .expect("first adoption should be accepted");

        let err = env
            .adopt_type_constructor(
                TypeConstructor::List,
                "Container",
                LIST_CONTAINER_ASSOCIATED_TYPES,
            )
            .expect_err("duplicate adoption should be rejected");

        assert!(matches!(err, TypeError::UnsupportedFeature(_)));
        assert!(
            err.to_string()
                .contains("duplicate built-in Container adoption"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn form_environment_rejects_duplicate_associated_type_names() {
        let duplicate_associated_types = [
            AssociatedTypeImplementation {
                assoc_name: "Item",
                resolver: AssociatedTypeResolver::Item,
            },
            AssociatedTypeImplementation {
                assoc_name: "Item",
                resolver: AssociatedTypeResolver::Mapped,
            },
        ];
        let mut env = FormEnvironment::new();

        let err = env
            .adopt_type_constructor(
                TypeConstructor::List,
                "Container",
                &duplicate_associated_types,
            )
            .expect_err("duplicate associated type name should be rejected");

        assert!(matches!(err, TypeError::UnsupportedFeature(_)));
        assert!(
            err.to_string().contains("duplicate associated type Item"),
            "unexpected error: {err}"
        );
    }
}
