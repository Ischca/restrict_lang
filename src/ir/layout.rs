//! Compile-time value layout descriptors for the Restrict IR.

use std::collections::HashMap;

use crate::type_checker::{ArrayLength, TypedType};

use super::{AbiId, FinalType, ScalarRepr, ValueRepr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutTable {
    layouts: Vec<LayoutDescriptor>,
    canonical: HashMap<LayoutKind, LayoutId>,
}

impl LayoutTable {
    pub fn new() -> Self {
        Self {
            layouts: Vec::new(),
            canonical: HashMap::new(),
        }
    }

    pub fn insert(&mut self, kind: LayoutKind) -> LayoutId {
        if matches!(kind, LayoutKind::Opaque(_)) {
            return self.push_descriptor(kind);
        }

        if let Some(id) = self.canonical.get(&kind) {
            return *id;
        }

        let id = self.push_descriptor(kind.clone());
        self.canonical.insert(kind, id);
        id
    }

    fn push_descriptor(&mut self, kind: LayoutKind) -> LayoutId {
        let id = LayoutId(self.layouts.len() as u32);
        self.layouts.push(LayoutDescriptor { id, kind });
        id
    }

    pub fn get(&self, id: LayoutId) -> Option<&LayoutDescriptor> {
        self.layouts.get(id.0 as usize)
    }

    pub fn descriptors(&self) -> &[LayoutDescriptor] {
        &self.layouts
    }

    pub fn value_repr_for_type(&mut self, final_type: &FinalType) -> ValueRepr {
        self.value_repr_for_type_with_record_fields(final_type, &|_, _| None)
    }

    pub fn value_repr_for_type_with_record_fields<F>(
        &mut self,
        final_type: &FinalType,
        record_fields: &F,
    ) -> ValueRepr
    where
        F: Fn(&str, &[TypedType]) -> Option<Vec<(String, TypedType)>> + ?Sized,
    {
        match final_type.as_typed_type() {
            TypedType::Unit => ValueRepr::Unit,
            TypedType::Int32 | TypedType::Boolean | TypedType::Char => {
                ValueRepr::Scalar(ScalarRepr::I32)
            }
            TypedType::Int64 => ValueRepr::Scalar(ScalarRepr::I64),
            TypedType::Float64 => ValueRepr::Scalar(ScalarRepr::F64),
            TypedType::String => {
                let id = self.insert(LayoutKind::String(StringLayout {
                    encoding: StringEncoding::Utf8,
                    header_words: 2,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::List(inner) => {
                let element = self.element_layout_with_record_fields(inner, record_fields);
                let id = self.insert(LayoutKind::List(ListLayout { element }));
                ValueRepr::Ref(id)
            }
            TypedType::Array(inner, length) => {
                let element = self.element_layout_with_record_fields(inner, record_fields);
                let id = self.insert(LayoutKind::Array(ArrayLayout {
                    element,
                    length: *length,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::Option(inner) => {
                let payload = self.element_layout_with_record_fields(inner, record_fields);
                let variants = vec![
                    SumVariantLayout {
                        tag: 0,
                        name: "None".to_string(),
                        payload: None,
                    },
                    SumVariantLayout {
                        tag: 1,
                        name: "Some".to_string(),
                        payload: Some(payload),
                    },
                ];
                let optimization_candidates = sum_optimization_candidates(&variants);
                let id = self.insert(LayoutKind::Sum(SumLayout {
                    variants,
                    strategy: SumStrategy::TaggedPayload,
                    optimization_candidates,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::Result(ok, err) => {
                let ok_payload = self.element_layout_with_record_fields(ok, record_fields);
                let err_payload = self.element_layout_with_record_fields(err, record_fields);
                let variants = vec![
                    SumVariantLayout {
                        tag: 0,
                        name: "Err".to_string(),
                        payload: Some(err_payload),
                    },
                    SumVariantLayout {
                        tag: 1,
                        name: "Ok".to_string(),
                        payload: Some(ok_payload),
                    },
                ];
                let optimization_candidates = sum_optimization_candidates(&variants);
                let id = self.insert(LayoutKind::Sum(SumLayout {
                    variants,
                    strategy: SumStrategy::TaggedPayload,
                    optimization_candidates,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::Record {
                name, type_args, ..
            } => {
                if is_range_int32_record(name, type_args) {
                    let id = self.insert(LayoutKind::Range(range_int32_layout()));
                    return ValueRepr::Ref(id);
                }

                let fields = record_fields(name, type_args)
                    .map(|fields| self.record_field_layouts(fields))
                    .unwrap_or_default();
                let type_args = type_args.iter().map(format_type_arg).collect();
                let id = self.insert(LayoutKind::Record(RecordLayout {
                    name: name.clone(),
                    type_args,
                    fields,
                    strategy: RecordStrategy::DescriptorManaged,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::Function {
                params,
                return_type,
            } => {
                let params = params
                    .iter()
                    .map(|param| self.element_layout_with_record_fields(param, record_fields))
                    .collect();
                let result =
                    Box::new(self.element_layout_with_record_fields(return_type, record_fields));
                let id = self.insert(LayoutKind::Closure(ClosureLayout {
                    abi: AbiId(0),
                    params,
                    result,
                    captures: Vec::new(),
                }));
                ValueRepr::Closure {
                    layout: id,
                    abi: AbiId(0),
                }
            }
            TypedType::Temporal { base_type, .. } => {
                let wrapped = FinalType::new((**base_type).clone())
                    .expect("temporal final type should contain a finalized base type");
                self.value_repr_for_type_with_record_fields(&wrapped, record_fields)
            }
            TypedType::TypeParam(_) | TypedType::InferVar(_) | TypedType::Projection { .. } => {
                let id = self.insert(LayoutKind::Opaque(OpaqueLayout {
                    reason: OpaqueReason::UnloweredGeneric,
                }));
                ValueRepr::Ref(id)
            }
        }
    }

    fn element_layout(&mut self, ty: &TypedType) -> ElementLayout {
        self.element_layout_with_record_fields(ty, &|_, _| None)
    }

    fn element_layout_with_record_fields<F>(
        &mut self,
        ty: &TypedType,
        record_fields: &F,
    ) -> ElementLayout
    where
        F: Fn(&str, &[TypedType]) -> Option<Vec<(String, TypedType)>> + ?Sized,
    {
        let repr = match FinalType::new(ty.clone()) {
            Ok(final_type) => {
                self.value_repr_for_type_with_record_fields(&final_type, record_fields)
            }
            Err(_) => {
                let id = self.insert(LayoutKind::Opaque(OpaqueLayout {
                    reason: OpaqueReason::UnfinalizedType,
                }));
                ValueRepr::Ref(id)
            }
        };

        ElementLayout {
            repr,
            size: size_of_repr(repr),
            align: align_of_repr(repr),
        }
    }

    fn record_field_layouts(&mut self, fields: Vec<(String, TypedType)>) -> Vec<FieldLayout> {
        let mut offset = 0;
        fields
            .into_iter()
            .map(|(name, ty)| {
                let element = self.element_layout(&ty);
                let field = FieldLayout {
                    name,
                    offset,
                    element,
                };
                offset += field.element.size;
                field
            })
            .collect()
    }
}

impl Default for LayoutTable {
    fn default() -> Self {
        Self::new()
    }
}

fn format_type_arg(ty: &TypedType) -> String {
    crate::type_checker::format_typed_type(ty)
}

fn is_range_int32_record(name: &str, type_args: &[TypedType]) -> bool {
    name == "Range" && type_args == [TypedType::Int32]
}

fn range_int32_layout() -> RangeLayout {
    let endpoint = ElementLayout {
        repr: ValueRepr::Scalar(ScalarRepr::I32),
        size: 4,
        align: 4,
    };
    RangeLayout {
        endpoint,
        start_offset: 0,
        end_offset: 4,
        size: 8,
        align: 4,
    }
}

fn sum_optimization_candidates(variants: &[SumVariantLayout]) -> Vec<SumOptimizationCandidate> {
    let mut candidates = Vec::new();

    let payload_variants = variants
        .iter()
        .filter_map(|variant| variant.payload.as_ref().map(|payload| (variant, payload)))
        .collect::<Vec<_>>();

    if let Some((variant, _)) = payload_variants
        .iter()
        .find(|(_, payload)| matches!(payload.repr, ValueRepr::Ref(_)))
    {
        if variants.iter().any(|variant| variant.payload.is_none()) {
            candidates.push(SumOptimizationCandidate::NullNiche {
                payload_variant: variant_identity(variant),
            });
        }
    }

    if !payload_variants.is_empty()
        && payload_variants
            .iter()
            .all(|(_, payload)| payload.repr.is_copy_scalar())
    {
        let payload_variants = payload_variants
            .into_iter()
            .map(|(variant, _)| variant_identity(variant))
            .collect::<Vec<_>>();
        candidates.push(SumOptimizationCandidate::ScalarPair {
            payload_variants: payload_variants.clone(),
        });
        candidates.push(SumOptimizationCandidate::ScalarLocal { payload_variants });
    }

    candidates
}

fn variant_identity(variant: &SumVariantLayout) -> SumVariantIdentity {
    SumVariantIdentity {
        tag: variant.tag,
        name: variant.name.clone(),
    }
}

fn size_of_repr(repr: ValueRepr) -> u32 {
    match repr {
        ValueRepr::Unit => 0,
        ValueRepr::Scalar(ScalarRepr::I32) | ValueRepr::Ref(_) | ValueRepr::Closure { .. } => 4,
        ValueRepr::Scalar(ScalarRepr::I64) | ValueRepr::Scalar(ScalarRepr::F64) => 8,
    }
}

fn align_of_repr(repr: ValueRepr) -> u32 {
    match repr {
        ValueRepr::Unit => 1,
        ValueRepr::Scalar(ScalarRepr::I64) | ValueRepr::Scalar(ScalarRepr::F64) => 8,
        ValueRepr::Scalar(ScalarRepr::I32) | ValueRepr::Ref(_) | ValueRepr::Closure { .. } => 4,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LayoutDescriptor {
    pub id: LayoutId,
    pub kind: LayoutKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LayoutKind {
    String(StringLayout),
    List(ListLayout),
    Array(ArrayLayout),
    Range(RangeLayout),
    Record(RecordLayout),
    Sum(SumLayout),
    Closure(ClosureLayout),
    Opaque(OpaqueLayout),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringLayout {
    pub encoding: StringEncoding,
    pub header_words: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StringEncoding {
    Utf8,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementLayout {
    pub repr: ValueRepr,
    pub size: u32,
    pub align: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ListLayout {
    pub element: ElementLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayLayout {
    pub element: ElementLayout,
    pub length: ArrayLength,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RangeLayout {
    pub endpoint: ElementLayout,
    pub start_offset: u32,
    pub end_offset: u32,
    pub size: u32,
    pub align: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordLayout {
    pub name: String,
    pub type_args: Vec<String>,
    pub fields: Vec<FieldLayout>,
    pub strategy: RecordStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldLayout {
    pub name: String,
    pub offset: u32,
    pub element: ElementLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RecordStrategy {
    DescriptorManaged,
    FieldsOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SumLayout {
    pub variants: Vec<SumVariantLayout>,
    pub strategy: SumStrategy,
    pub optimization_candidates: Vec<SumOptimizationCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SumVariantLayout {
    pub tag: u32,
    pub name: String,
    pub payload: Option<ElementLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SumStrategy {
    TaggedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SumVariantIdentity {
    pub tag: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SumOptimizationCandidate {
    NullNiche {
        payload_variant: SumVariantIdentity,
    },
    ScalarPair {
        payload_variants: Vec<SumVariantIdentity>,
    },
    ScalarLocal {
        payload_variants: Vec<SumVariantIdentity>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClosureLayout {
    pub abi: AbiId,
    pub params: Vec<ElementLayout>,
    pub result: Box<ElementLayout>,
    pub captures: Vec<ElementLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpaqueLayout {
    pub reason: OpaqueReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OpaqueReason {
    UnfinalizedType,
    UnloweredGeneric,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_checker::TypedType;

    fn sum_variant(tag: u32, name: &str) -> SumVariantIdentity {
        SumVariantIdentity {
            tag,
            name: name.to_string(),
        }
    }

    #[test]
    fn list_layout_records_element_size_and_alignment() {
        let final_type = FinalType::new(TypedType::List(Box::new(TypedType::Int64))).unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("List should lower to a typed ref");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::List(layout) = &descriptor.kind else {
            panic!("expected List layout");
        };
        assert_eq!(layout.element.size, 8);
        assert_eq!(layout.element.align, 8);
    }

    #[test]
    fn option_layout_keeps_logical_tags() {
        let final_type = FinalType::new(TypedType::Option(Box::new(TypedType::Int32))).unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Option should lower to a typed ref initially");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Sum(layout) = &descriptor.kind else {
            panic!("expected Sum layout");
        };

        assert_eq!(layout.variants[0].name, "None");
        assert_eq!(layout.variants[0].tag, 0);
        assert_eq!(layout.variants[1].name, "Some");
        assert_eq!(layout.variants[1].tag, 1);
        assert_eq!(layout.strategy, SumStrategy::TaggedPayload);
        assert!(layout
            .optimization_candidates
            .contains(&SumOptimizationCandidate::ScalarPair {
                payload_variants: vec![sum_variant(1, "Some")]
            }));
        assert!(layout
            .optimization_candidates
            .contains(&SumOptimizationCandidate::ScalarLocal {
                payload_variants: vec![sum_variant(1, "Some")]
            }));
    }

    #[test]
    fn array_layout_preserves_known_length() {
        let final_type = FinalType::new(TypedType::Array(
            Box::new(TypedType::Int32),
            ArrayLength::Known(3),
        ))
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Array should lower to a typed ref");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Array(layout) = &descriptor.kind else {
            panic!("expected Array layout");
        };
        assert_eq!(layout.length, ArrayLength::Known(3));
    }

    #[test]
    fn option_ref_layout_records_null_niche_candidate_without_changing_strategy() {
        let final_type = FinalType::new(TypedType::Option(Box::new(TypedType::String))).unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Option should lower to a typed ref initially");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Sum(layout) = &descriptor.kind else {
            panic!("expected Sum layout");
        };

        assert_eq!(layout.strategy, SumStrategy::TaggedPayload);
        assert!(layout
            .optimization_candidates
            .contains(&SumOptimizationCandidate::NullNiche {
                payload_variant: sum_variant(1, "Some")
            }));
        assert!(!layout
            .optimization_candidates
            .iter()
            .any(|candidate| matches!(candidate, SumOptimizationCandidate::ScalarPair { .. })));
    }

    #[test]
    fn option_function_layout_does_not_claim_null_niche_candidate() {
        let final_type = FinalType::new(TypedType::Option(Box::new(TypedType::Function {
            params: vec![TypedType::Int32],
            return_type: Box::new(TypedType::Int32),
        })))
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Option should lower to a typed ref initially");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Sum(layout) = &descriptor.kind else {
            panic!("expected Sum layout");
        };

        assert_eq!(layout.strategy, SumStrategy::TaggedPayload);
        assert!(layout.optimization_candidates.is_empty());
    }

    #[test]
    fn range_int32_layout_records_endpoint_offsets() {
        let final_type = FinalType::new(TypedType::Record {
            name: "Range".to_string(),
            type_args: vec![TypedType::Int32],
            frozen: false,
            hash: None,
            parent_hash: None,
        })
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Range<Int32> should lower to a typed ref");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Range(layout) = &descriptor.kind else {
            panic!("expected Range layout");
        };
        assert_eq!(layout.endpoint.repr, ValueRepr::Scalar(ScalarRepr::I32));
        assert_eq!(layout.endpoint.size, 4);
        assert_eq!(layout.endpoint.align, 4);
        assert_eq!(layout.start_offset, 0);
        assert_eq!(layout.end_offset, 4);
        assert_eq!(layout.size, 8);
        assert_eq!(layout.align, 4);
    }

    #[test]
    fn record_layout_uses_provider_field_order_and_offsets() {
        let final_type = FinalType::new(TypedType::Record {
            name: "ReleaseScore".to_string(),
            type_args: Vec::new(),
            frozen: false,
            hash: None,
            parent_hash: None,
        })
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type_with_record_fields(&final_type, &|name, type_args| {
            assert_eq!(name, "ReleaseScore");
            assert!(type_args.is_empty());
            Some(vec![
                ("value".to_string(), TypedType::Int32),
                ("label".to_string(), TypedType::String),
                ("weight".to_string(), TypedType::Int64),
            ])
        });
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Record should lower to a typed ref");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Record(layout) = &descriptor.kind else {
            panic!("expected Record layout");
        };

        assert_eq!(layout.name, "ReleaseScore");
        assert_eq!(layout.fields.len(), 3);
        assert_eq!(layout.fields[0].name, "value");
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(
            layout.fields[0].element.repr,
            ValueRepr::Scalar(ScalarRepr::I32)
        );
        assert_eq!(layout.fields[1].name, "label");
        assert_eq!(layout.fields[1].offset, 4);
        assert!(matches!(layout.fields[1].element.repr, ValueRepr::Ref(_)));
        assert_eq!(layout.fields[2].name, "weight");
        assert_eq!(layout.fields[2].offset, 8);
        assert_eq!(
            layout.fields[2].element.repr,
            ValueRepr::Scalar(ScalarRepr::I64)
        );
    }

    #[test]
    fn generic_record_layout_applies_provider_type_args() {
        let final_type = FinalType::new(TypedType::Record {
            name: "Box".to_string(),
            type_args: vec![TypedType::Int64],
            frozen: false,
            hash: None,
            parent_hash: None,
        })
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type_with_record_fields(&final_type, &|name, type_args| {
            assert_eq!(name, "Box");
            Some(vec![("value".to_string(), type_args[0].clone())])
        });
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Record should lower to a typed ref");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Record(layout) = &descriptor.kind else {
            panic!("expected Record layout");
        };

        assert_eq!(layout.type_args, vec!["Int64"]);
        assert_eq!(layout.fields.len(), 1);
        assert_eq!(
            layout.fields[0].element.repr,
            ValueRepr::Scalar(ScalarRepr::I64)
        );
        assert_eq!(layout.fields[0].element.size, 8);
    }

    #[test]
    fn result_layout_keeps_err_zero_ok_one_tags() {
        let final_type = FinalType::new(TypedType::Result(
            Box::new(TypedType::Int32),
            Box::new(TypedType::String),
        ))
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Result should lower to a typed ref initially");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Sum(layout) = &descriptor.kind else {
            panic!("expected Sum layout");
        };

        assert_eq!(layout.variants[0].name, "Err");
        assert_eq!(layout.variants[0].tag, 0);
        assert_eq!(layout.variants[1].name, "Ok");
        assert_eq!(layout.variants[1].tag, 1);
        assert_eq!(layout.strategy, SumStrategy::TaggedPayload);
    }

    #[test]
    fn result_scalar_layout_records_scalar_pair_candidate_without_changing_strategy() {
        let final_type = FinalType::new(TypedType::Result(
            Box::new(TypedType::Int32),
            Box::new(TypedType::Boolean),
        ))
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Result should lower to a typed ref initially");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Sum(layout) = &descriptor.kind else {
            panic!("expected Sum layout");
        };

        assert_eq!(layout.strategy, SumStrategy::TaggedPayload);
        assert!(layout
            .optimization_candidates
            .contains(&SumOptimizationCandidate::ScalarPair {
                payload_variants: vec![sum_variant(0, "Err"), sum_variant(1, "Ok")]
            }));
        assert!(layout
            .optimization_candidates
            .contains(&SumOptimizationCandidate::ScalarLocal {
                payload_variants: vec![sum_variant(0, "Err"), sum_variant(1, "Ok")]
            }));
        assert!(!layout
            .optimization_candidates
            .iter()
            .any(|candidate| matches!(candidate, SumOptimizationCandidate::NullNiche { .. })));
    }

    #[test]
    fn result_ref_and_scalar_layout_does_not_claim_null_niche_candidate() {
        let final_type = FinalType::new(TypedType::Result(
            Box::new(TypedType::String),
            Box::new(TypedType::Int32),
        ))
        .unwrap();
        let mut table = LayoutTable::new();
        let repr = table.value_repr_for_type(&final_type);
        let ValueRepr::Ref(layout_id) = repr else {
            panic!("Result should lower to a typed ref initially");
        };

        let descriptor = table.get(layout_id).unwrap();
        let LayoutKind::Sum(layout) = &descriptor.kind else {
            panic!("expected Sum layout");
        };

        assert_eq!(layout.strategy, SumStrategy::TaggedPayload);
        assert!(layout.optimization_candidates.is_empty());
    }

    #[test]
    fn layout_table_reuses_identical_list_layouts() {
        let final_type = FinalType::new(TypedType::List(Box::new(TypedType::Int32))).unwrap();
        let mut table = LayoutTable::new();

        let first = table.value_repr_for_type(&final_type);
        let second = table.value_repr_for_type(&final_type);

        assert_eq!(first, second);
        assert_eq!(table.descriptors().len(), 1);
    }

    #[test]
    fn layout_table_reuses_nested_composite_layouts() {
        let final_type = FinalType::new(TypedType::List(Box::new(TypedType::String))).unwrap();
        let mut table = LayoutTable::new();

        let first = table.value_repr_for_type(&final_type);
        let second = table.value_repr_for_type(&final_type);

        assert_eq!(first, second);
        assert_eq!(
            table
                .descriptors()
                .iter()
                .filter(|descriptor| matches!(descriptor.kind, LayoutKind::String(_)))
                .count(),
            1
        );
        assert_eq!(
            table
                .descriptors()
                .iter()
                .filter(|descriptor| matches!(descriptor.kind, LayoutKind::List(_)))
                .count(),
            1
        );
        assert_eq!(table.descriptors().len(), 2);
    }

    #[test]
    fn layout_table_reuses_closure_layouts() {
        let final_type = FinalType::new(TypedType::Function {
            params: vec![TypedType::String],
            return_type: Box::new(TypedType::List(Box::new(TypedType::Int32))),
        })
        .unwrap();
        let mut table = LayoutTable::new();

        let first = table.value_repr_for_type(&final_type);
        let second = table.value_repr_for_type(&final_type);

        assert_eq!(first, second);
        assert_eq!(
            table
                .descriptors()
                .iter()
                .filter(|descriptor| matches!(descriptor.kind, LayoutKind::Closure(_)))
                .count(),
            1
        );
    }

    #[test]
    fn layout_table_keeps_distinct_array_lengths() {
        let mut table = LayoutTable::new();
        let array_three = FinalType::new(TypedType::Array(
            Box::new(TypedType::Int32),
            ArrayLength::Known(3),
        ))
        .unwrap();
        let array_four = FinalType::new(TypedType::Array(
            Box::new(TypedType::Int32),
            ArrayLength::Known(4),
        ))
        .unwrap();

        let three_first = table.value_repr_for_type(&array_three);
        let three_second = table.value_repr_for_type(&array_three);
        let four = table.value_repr_for_type(&array_four);

        assert_eq!(three_first, three_second);
        assert_ne!(three_first, four);
    }

    #[test]
    fn layout_table_keeps_result_variant_roles_distinct() {
        let mut table = LayoutTable::new();
        let int_or_string = FinalType::new(TypedType::Result(
            Box::new(TypedType::Int32),
            Box::new(TypedType::String),
        ))
        .unwrap();
        let string_or_int = FinalType::new(TypedType::Result(
            Box::new(TypedType::String),
            Box::new(TypedType::Int32),
        ))
        .unwrap();

        let first = table.value_repr_for_type(&int_or_string);
        let second = table.value_repr_for_type(&int_or_string);
        let swapped = table.value_repr_for_type(&string_or_int);

        assert_eq!(first, second);
        assert_ne!(first, swapped);
    }

    #[test]
    fn opaque_layouts_are_not_canonical_without_provenance() {
        let mut table = LayoutTable::new();
        let generic = FinalType::new(TypedType::TypeParam("T".to_string())).unwrap();

        let first = table.value_repr_for_type(&generic);
        let second = table.value_repr_for_type(&generic);

        assert_ne!(first, second);
        assert_eq!(
            table
                .descriptors()
                .iter()
                .filter(|descriptor| matches!(descriptor.kind, LayoutKind::Opaque(_)))
                .count(),
            2
        );
    }
}
