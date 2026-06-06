//! Compile-time value layout descriptors for the Restrict IR.

use crate::type_checker::{ArrayLength, TypedType};

use super::{AbiId, FinalType, ScalarRepr, ValueRepr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutTable {
    layouts: Vec<LayoutDescriptor>,
}

impl LayoutTable {
    pub fn new() -> Self {
        Self {
            layouts: Vec::new(),
        }
    }

    pub fn insert(&mut self, kind: LayoutKind) -> LayoutId {
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
                let element = self.element_layout(inner);
                let id = self.insert(LayoutKind::List(ListLayout { element }));
                ValueRepr::Ref(id)
            }
            TypedType::Array(inner, length) => {
                let element = self.element_layout(inner);
                let id = self.insert(LayoutKind::Array(ArrayLayout {
                    element,
                    length: *length,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::Option(inner) => {
                let payload = self.element_layout(inner);
                let id = self.insert(LayoutKind::Sum(SumLayout {
                    variants: vec![
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
                    ],
                    strategy: SumStrategy::TaggedPayload,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::Result(ok, err) => {
                let ok_payload = self.element_layout(ok);
                let err_payload = self.element_layout(err);
                let id = self.insert(LayoutKind::Sum(SumLayout {
                    variants: vec![
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
                    ],
                    strategy: SumStrategy::TaggedPayload,
                }));
                ValueRepr::Ref(id)
            }
            TypedType::Record {
                name, type_args, ..
            } => {
                let type_args = type_args.iter().map(format_type_arg).collect();
                let id = self.insert(LayoutKind::Record(RecordLayout {
                    name: name.clone(),
                    type_args,
                    fields: Vec::new(),
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
                    .map(|param| self.element_layout(param))
                    .collect();
                let result = Box::new(self.element_layout(return_type));
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
                self.value_repr_for_type(&wrapped)
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
        let repr = match FinalType::new(ty.clone()) {
            Ok(final_type) => self.value_repr_for_type(&final_type),
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
}

impl Default for LayoutTable {
    fn default() -> Self {
        Self::new()
    }
}

fn format_type_arg(ty: &TypedType) -> String {
    crate::type_checker::format_typed_type(ty)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDescriptor {
    pub id: LayoutId,
    pub kind: LayoutKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutKind {
    String(StringLayout),
    List(ListLayout),
    Array(ArrayLayout),
    Record(RecordLayout),
    Sum(SumLayout),
    Closure(ClosureLayout),
    Opaque(OpaqueLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLayout {
    pub encoding: StringEncoding,
    pub header_words: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringEncoding {
    Utf8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementLayout {
    pub repr: ValueRepr,
    pub size: u32,
    pub align: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLayout {
    pub element: ElementLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayLayout {
    pub element: ElementLayout,
    pub length: ArrayLength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLayout {
    pub name: String,
    pub type_args: Vec<String>,
    pub fields: Vec<FieldLayout>,
    pub strategy: RecordStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    pub name: String,
    pub offset: u32,
    pub element: ElementLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordStrategy {
    DescriptorManaged,
    FieldsOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SumLayout {
    pub variants: Vec<SumVariantLayout>,
    pub strategy: SumStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SumVariantLayout {
    pub tag: u32,
    pub name: String,
    pub payload: Option<ElementLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SumStrategy {
    TaggedPayload,
    NicheCandidate,
    ScalarPairCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureLayout {
    pub abi: AbiId,
    pub params: Vec<ElementLayout>,
    pub result: Box<ElementLayout>,
    pub captures: Vec<ElementLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueLayout {
    pub reason: OpaqueReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpaqueReason {
    UnfinalizedType,
    UnloweredGeneric,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_checker::TypedType;

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
    }
}
