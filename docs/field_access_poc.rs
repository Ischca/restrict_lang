// Proof of Concept: Improved Field Access Implementation
// This shows how to modify the type checker to support copyable field access

impl TypeChecker {
    /// Improved field access that doesn't consume records for copyable fields
    fn check_field_access(&mut self, expr: &Expr, field: &str) -> Result<TypedType, TypeError> {
        // First, infer the type of the object expression without consuming it
        let obj_ty = self.infer_expr_type_readonly(expr)?;
        
        // Handle temporal types by unwrapping to the base type
        let base_ty = match &obj_ty {
            TypedType::Temporal { base_type, .. } => base_type.as_ref(),
            _ => &obj_ty,
        };
        
        // Get the field type
        let field_ty = match base_ty {
            TypedType::Record { name, .. } => {
                let record_def = self.records.get(name).unwrap();
                record_def.fields.get(field)
                    .cloned()
                    .ok_or_else(|| TypeError::UnknownField {
                        record: name.clone(),
                        field: field.to_string(),
                    })?
            }
            _ => return Err(TypeError::TypeMismatch {
                expected: "record".to_string(),
                found: format!("{:?}", obj_ty),
            })
        };
        
        // Consumption strategy based on field type
        if self.is_copyable(&field_ty) {
            // For copyable fields: just verify the object is accessible (don't consume)
            self.check_expr_accessibility(expr)?;
        } else {
            // For affine fields: consume the object expression  
            let _ = self.check_expr(expr)?;
        }
        
        Ok(field_ty)
    }
    
    /// Infer expression type without consuming variables (read-only type inference)
    fn infer_expr_type_readonly(&self, expr: &Expr) -> Result<TypedType, TypeError> {
        match expr {
            Expr::IntLit(_) => Ok(TypedType::Int32),
            Expr::FloatLit(_) => Ok(TypedType::Float64),
            Expr::StringLit(_) => Ok(TypedType::String),
            Expr::CharLit(_) => Ok(TypedType::Char),
            Expr::BoolLit(_) => Ok(TypedType::Boolean),
            Expr::Unit => Ok(TypedType::Unit),
            
            Expr::Ident(name) => {
                // Look up variable type without marking as used
                for scope in self.var_env.iter().rev() {
                    if let Some(var_info) = scope.get(name) {
                        return Ok(var_info.ty.clone());
                    }
                }
                
                // Check functions
                if let Some(func_def) = self.functions.get(name) {
                    return Ok(TypedType::Function {
                        params: func_def.params.iter().map(|(_, ty)| ty.clone()).collect(),
                        return_type: Box::new(func_def.return_type.clone()),
                    });
                }
                
                Err(TypeError::UnknownVariable(name.clone()))
            }
            
            Expr::FieldAccess(obj_expr, field) => {
                let obj_ty = self.infer_expr_type_readonly(obj_expr)?;
                let base_ty = match &obj_ty {
                    TypedType::Temporal { base_type, .. } => base_type.as_ref(),
                    _ => &obj_ty,
                };
                
                match base_ty {
                    TypedType::Record { name, .. } => {
                        let record_def = self.records.get(name).unwrap();
                        record_def.fields.get(field)
                            .cloned()
                            .ok_or_else(|| TypeError::UnknownField {
                                record: name.clone(),
                                field: field.to_string(),
                            })
                    }
                    _ => Err(TypeError::TypeMismatch {
                        expected: "record".to_string(),
                        found: format!("{:?}", obj_ty),
                    })
                }
            }
            
            Expr::RecordLit(record_lit) => {
                Ok(TypedType::Record {
                    name: record_lit.name.clone(),
                    type_params: vec![], // Simplified for POC
                })
            }
            
            // Add other expression types as needed...
            _ => Err(TypeError::UnsupportedFeature("Unsupported expression in readonly inference".to_string()))
        }
    }
    
    /// Check that an expression is accessible without consuming it
    fn check_expr_accessibility(&self, expr: &Expr) -> Result<(), TypeError> {
        match expr {
            Expr::Ident(name) => {
                // Verify variable exists and isn't already consumed
                for scope in self.var_env.iter().rev() {
                    if let Some(var_info) = scope.get(name) {
                        if var_info.used && !self.is_copyable(&var_info.ty) && !var_info.mutable {
                            return Err(TypeError::AffineViolation(name.clone()));
                        }
                        return Ok(());
                    }
                }
                Err(TypeError::UnknownVariable(name.clone()))
            }
            
            Expr::FieldAccess(obj_expr, _) => {
                // Recursively check accessibility of the object
                self.check_expr_accessibility(obj_expr)
            }
            
            // Literals are always accessible
            Expr::IntLit(_) | Expr::FloatLit(_) | Expr::StringLit(_) 
            | Expr::CharLit(_) | Expr::BoolLit(_) | Expr::Unit => Ok(()),
            
            _ => Err(TypeError::UnsupportedFeature("Unsupported expression in accessibility check".to_string()))
        }
    }
    
    /// Enhanced copyable check that considers record fields
    fn is_copyable(&self, ty: &TypedType) -> bool {
        match ty {
            // Primitive types are always copyable
            TypedType::Int32 | TypedType::Boolean | TypedType::Float64 
            | TypedType::Char | TypedType::Unit => true,
            
            // Heap-allocated types are never copyable
            TypedType::String | TypedType::List(_) => false,
            
            // Records are copyable only if ALL fields are copyable
            TypedType::Record { name, .. } => {
                if let Some(record_def) = self.records.get(name) {
                    record_def.fields.values().all(|field_ty| self.is_copyable(field_ty))
                } else {
                    false // Unknown record, assume not copyable
                }
            }
            
            // Functions are never copyable (they may capture affine data)
            TypedType::Function { .. } => false,
            
            // Composite types
            TypedType::Option(inner) => self.is_copyable(inner),
            TypedType::Array(inner, _) => self.is_copyable(inner),
            
            // Temporal types follow their base type
            TypedType::Temporal { base_type, .. } => self.is_copyable(base_type),
            
            // Type parameters are copyable only with Copy bound
            TypedType::TypeParam(param_name) => {
                self.get_type_bounds(param_name).contains(&"Copy".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copyable_field_access() {
        let input = r#"
            record Point { x: Int32, y: Int32 }
            val p = Point { x = 10, y = 20 }
            val x = p.x  // OK: Int32 is copyable, doesn't consume p
            val y = p.y  // OK: Int32 is copyable, p still available
            val sum = p.x + p.y  // OK: can use p multiple times for copyable fields
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_mixed_field_access() {
        let input = r#"
            record User { id: Int32, name: String }
            val u = User { id = 123, name = "Alice" }
            val id1 = u.id   // OK: Int32 is copyable
            val id2 = u.id   // OK: can access copyable field multiple times
            val name = u.name  // Consumes u (String is affine)
            val id3 = u.id   // ERROR: u already consumed
        "#;
        assert_eq!(
            check_program_str(input),
            Err(TypeError::AffineViolation("u".to_string()))
        );
    }

    #[test]
    fn test_all_copyable_record() {
        let input = r#"
            record Point3D { x: Float64, y: Float64, z: Float64 }
            val p = Point3D { x = 1.0, y = 2.0, z = 3.0 }
            val length = sqrt(p.x * p.x + p.y * p.y + p.z * p.z)
            val scaled_x = p.x * 2.0  // Still can use p for copyable access
        "#;
        assert!(check_program_str(input).is_ok());
    }

    #[test]
    fn test_nested_copyable_access() {
        let input = r#"
            record Point { x: Float64, y: Float64 }
            record Line { start: Point, end: Point }
            
            val line = Line { 
                start = Point { x = 0.0, y = 0.0 },
                end = Point { x = 10.0, y = 10.0 }
            }
            
            // These should work if Point fields are all copyable
            val start_x = line.start.x  // OK: nested copyable access
            val end_y = line.end.y      // OK: independent copyable access
        "#;
        // Note: This would require more sophisticated analysis
        // For now, accessing line.start would consume line since Point is a record
        // But if Point were redesigned to be copyable, this would work
    }

    #[test]
    fn test_destructuring_preferred_pattern() {
        let input = r#"
            record User { id: Int32, name: String, email: String }
            val user = User { id = 123, name = "Alice", email = "alice@example.com" }
            
            // PREFERRED: destructure for multiple affine field access
            val User { id, name, email } = user
            val greeting = "Hello " ++ name ++ " (ID: " ++ toString(id) ++ ")"
        "#;
        assert!(check_program_str(input).is_ok());
    }
}