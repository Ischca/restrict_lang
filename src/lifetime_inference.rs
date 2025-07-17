//! # Lifetime Inference Module
//!
//! Implements automatic lifetime inference for Temporal Affine Types.
//! This module analyzes the usage patterns of temporal values and infers
//! appropriate lifetime parameters, reducing the need for explicit annotations.
//!
//! ## Algorithm Overview
//!
//! 1. **Collection Phase**: Gather all temporal values and their usage sites
//! 2. **Constraint Generation**: Create lifetime constraints based on usage patterns
//! 3. **Constraint Solving**: Solve constraints to determine lifetime relationships
//! 4. **Annotation**: Apply inferred lifetimes to the AST

use std::collections::{HashMap, HashSet};
use crate::ast::*;

/// Represents a lifetime variable in the inference system
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LifetimeVar {
    /// Named lifetime parameter (e.g., ~f, ~io)
    Named(String),
    /// Anonymous lifetime generated during inference
    Anonymous(usize),
    /// Static lifetime (lives for entire program)
    Static,
}

/// A constraint between two lifetimes
#[derive(Debug, Clone)]
pub enum LifetimeConstraint {
    /// `a` outlives `b` (a ⊇ b)
    Outlives(LifetimeVar, LifetimeVar),
    /// `a` is equal to `b`
    Equal(LifetimeVar, LifetimeVar),
    /// `a` is within `b` (a ⊆ b)
    Within(LifetimeVar, LifetimeVar),
}

/// Context for lifetime inference
#[derive(Debug)]
pub struct LifetimeInferenceContext {
    /// Counter for generating anonymous lifetimes
    anonymous_counter: usize,
    /// All lifetime variables in the system
    lifetime_vars: HashSet<LifetimeVar>,
    /// Constraints between lifetimes
    constraints: Vec<LifetimeConstraint>,
    /// Mapping from expressions to their lifetimes
    expr_lifetimes: HashMap<ExprId, LifetimeVar>,
    /// Current scope's lifetime
    current_scope_lifetime: Option<LifetimeVar>,
    /// Stack of lifetime scopes
    scope_stack: Vec<LifetimeVar>,
}

/// Unique identifier for expressions
type ExprId = usize;

impl LifetimeInferenceContext {
    pub fn new() -> Self {
        Self {
            anonymous_counter: 0,
            lifetime_vars: HashSet::new(),
            constraints: Vec::new(),
            expr_lifetimes: HashMap::new(),
            current_scope_lifetime: None,
            scope_stack: Vec::new(),
        }
    }
    
    /// Generate a fresh anonymous lifetime
    fn fresh_lifetime(&mut self) -> LifetimeVar {
        let lifetime = LifetimeVar::Anonymous(self.anonymous_counter);
        self.anonymous_counter += 1;
        self.lifetime_vars.insert(lifetime.clone());
        lifetime
    }
    
    /// Add a lifetime constraint
    fn add_constraint(&mut self, constraint: LifetimeConstraint) {
        self.constraints.push(constraint);
    }
    
    /// Enter a new lifetime scope
    fn enter_scope(&mut self, lifetime: LifetimeVar) {
        if let Some(current) = &self.current_scope_lifetime {
            self.scope_stack.push(current.clone());
        }
        self.current_scope_lifetime = Some(lifetime);
    }
    
    /// Exit the current lifetime scope
    fn exit_scope(&mut self) {
        if let Some(parent) = self.scope_stack.pop() {
            self.current_scope_lifetime = Some(parent);
        } else {
            self.current_scope_lifetime = None;
        }
    }
}

/// Main lifetime inference algorithm
pub struct LifetimeInference {
    context: LifetimeInferenceContext,
}

impl LifetimeInference {
    pub fn new() -> Self {
        Self {
            context: LifetimeInferenceContext::new(),
        }
    }
    
    /// Infer lifetimes for a program
    pub fn infer_program(&mut self, program: &Program) -> Result<LifetimeAnnotations, String> {
        // Phase 1: Collect temporal values and usage sites
        for decl in &program.declarations {
            self.collect_from_decl(decl)?;
        }
        
        // Phase 2: Generate constraints
        self.generate_constraints(program)?;
        
        // Phase 3: Solve constraints
        let solution = self.solve_constraints()?;
        
        // Phase 4: Apply solution to create annotations
        Ok(self.create_annotations(solution))
    }
    
    /// Collect temporal values from a declaration
    fn collect_from_decl(&mut self, decl: &TopDecl) -> Result<(), String> {
        match decl {
            TopDecl::Function(func) => self.collect_from_function(func),
            TopDecl::Record(record) => self.collect_from_record(record),
            _ => Ok(()),
        }
    }
    
    /// Collect temporal values from a function
    fn collect_from_function(&mut self, func: &FunDecl) -> Result<(), String> {
        // Create lifetime scope for function
        let func_lifetime = if func.type_params.iter().any(|p| p.is_temporal) {
            // Use existing temporal parameters
            LifetimeVar::Named(func.type_params[0].name.clone())
        } else {
            // Create anonymous lifetime for function scope
            self.context.fresh_lifetime()
        };
        
        self.context.enter_scope(func_lifetime);
        
        // Analyze function body
        self.collect_from_block(&func.body)?;
        
        self.context.exit_scope();
        Ok(())
    }
    
    /// Collect temporal values from a record
    fn collect_from_record(&mut self, record: &RecordDecl) -> Result<(), String> {
        // Register temporal parameters
        for param in &record.type_params {
            if param.is_temporal {
                let lifetime = LifetimeVar::Named(param.name.clone());
                self.context.lifetime_vars.insert(lifetime);
            }
        }
        
        // Register temporal constraints
        for constraint in &record.temporal_constraints {
            let inner = LifetimeVar::Named(constraint.inner.clone());
            let outer = LifetimeVar::Named(constraint.outer.clone());
            self.context.add_constraint(LifetimeConstraint::Within(inner, outer));
        }
        
        Ok(())
    }
    
    /// Collect from a block expression
    fn collect_from_block(&mut self, block: &BlockExpr) -> Result<(), String> {
        for stmt in &block.statements {
            match stmt {
                Stmt::Binding(bind_decl) => {
                    self.collect_from_expr(&bind_decl.value)?;
                    // If binding a temporal value, track its lifetime
                    if self.is_temporal_expr(&bind_decl.value) {
                        // TODO: Track pattern bindings
                    }
                }
                Stmt::Assignment(assign_stmt) => {
                    self.collect_from_expr(&assign_stmt.value)?;
                }
                Stmt::Expr(expr) => {
                    self.collect_from_expr(expr)?;
                }
            }
        }
        
        if let Some(expr) = &block.expr {
            self.collect_from_expr(expr)?;
        }
        
        Ok(())
    }
    
    /// Collect from an expression
    fn collect_from_expr(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::RecordLit(record_lit) => {
                // Check if this is a temporal record
                if self.is_temporal_record(&record_lit.name) {
                    let _lifetime = self.context.current_scope_lifetime
                        .clone()
                        .unwrap_or_else(|| self.context.fresh_lifetime());
                    // TODO: Store expr_id -> lifetime mapping
                }
                
                // Recursively collect from field values
                for field in &record_lit.fields {
                    self.collect_from_expr(&field.value)?;
                }
            }
            Expr::Call(call_expr) => {
                for arg in &call_expr.args {
                    self.collect_from_expr(arg)?;
                }
            }
            Expr::Block(block) => {
                self.collect_from_block(block)?;
            }
            Expr::Then(then_expr) => {
                self.collect_from_expr(&then_expr.condition)?;
                self.collect_from_block(&then_expr.then_block)?;
                if let Some(else_block) = &then_expr.else_block {
                    self.collect_from_block(else_block)?;
                }
            }
            Expr::Binary(binary_expr) => {
                self.collect_from_expr(&binary_expr.left)?;
                self.collect_from_expr(&binary_expr.right)?;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Check if an expression produces a temporal value
    fn is_temporal_expr(&self, _expr: &Expr) -> bool {
        // TODO: Implement based on type information
        false
    }
    
    /// Check if a record type is temporal
    fn is_temporal_record(&self, _name: &str) -> bool {
        // TODO: Implement based on record definitions
        false
    }
    
    /// Generate lifetime constraints from the program
    fn generate_constraints(&mut self, _program: &Program) -> Result<(), String> {
        // TODO: Implement constraint generation
        // 1. Function return values must not outlive function scope
        // 2. Field accesses must respect record lifetimes
        // 3. Pattern matches must respect lifetime bounds
        Ok(())
    }
    
    /// Solve lifetime constraints
    fn solve_constraints(&self) -> Result<LifetimeSolution, String> {
        let mut solution = LifetimeSolution::new();
        
        // Simple constraint solver using fixed-point iteration
        let mut changed = true;
        while changed {
            changed = false;
            
            for constraint in &self.context.constraints {
                match constraint {
                    LifetimeConstraint::Within(inner, outer) => {
                        // inner ⊆ outer means inner cannot outlive outer
                        if !solution.is_within(inner, outer) {
                            solution.add_within(inner.clone(), outer.clone());
                            changed = true;
                        }
                    }
                    LifetimeConstraint::Equal(a, b) => {
                        // a = b means they have the same lifetime
                        if !solution.are_equal(a, b) {
                            solution.unify(a.clone(), b.clone());
                            changed = true;
                        }
                    }
                    LifetimeConstraint::Outlives(a, b) => {
                        // a ⊇ b means a outlives b
                        if !solution.outlives(a, b) {
                            solution.add_outlives(a.clone(), b.clone());
                            changed = true;
                        }
                    }
                }
            }
        }
        
        // Check for contradictions
        solution.validate()?;
        
        Ok(solution)
    }
    
    /// Create lifetime annotations from the solution
    fn create_annotations(&self, solution: LifetimeSolution) -> LifetimeAnnotations {
        LifetimeAnnotations {
            expr_lifetimes: HashMap::new(),
            inferred_lifetimes: solution.get_all_lifetimes(),
        }
    }
}

/// Solution to lifetime constraints
#[derive(Debug)]
struct LifetimeSolution {
    /// Equivalence classes of lifetimes
    equiv_classes: HashMap<LifetimeVar, LifetimeVar>,
    /// Within relationships (inner -> set of outers)
    within_relations: HashMap<LifetimeVar, HashSet<LifetimeVar>>,
    /// Outlives relationships (outer -> set of inners)
    outlives_relations: HashMap<LifetimeVar, HashSet<LifetimeVar>>,
}

impl LifetimeSolution {
    fn new() -> Self {
        Self {
            equiv_classes: HashMap::new(),
            within_relations: HashMap::new(),
            outlives_relations: HashMap::new(),
        }
    }
    
    fn is_within(&self, inner: &LifetimeVar, outer: &LifetimeVar) -> bool {
        self.within_relations
            .get(inner)
            .map(|outers| outers.contains(outer))
            .unwrap_or(false)
    }
    
    fn add_within(&mut self, inner: LifetimeVar, outer: LifetimeVar) {
        self.within_relations
            .entry(inner)
            .or_insert_with(HashSet::new)
            .insert(outer);
    }
    
    fn are_equal(&self, a: &LifetimeVar, b: &LifetimeVar) -> bool {
        self.find_root(a) == self.find_root(b)
    }
    
    fn unify(&mut self, a: LifetimeVar, b: LifetimeVar) {
        let root_a = self.find_root(&a);
        let root_b = self.find_root(&b);
        if root_a != root_b {
            self.equiv_classes.insert(root_a, root_b);
        }
    }
    
    fn outlives(&self, a: &LifetimeVar, b: &LifetimeVar) -> bool {
        self.outlives_relations
            .get(a)
            .map(|inners| inners.contains(b))
            .unwrap_or(false)
    }
    
    fn add_outlives(&mut self, a: LifetimeVar, b: LifetimeVar) {
        self.outlives_relations
            .entry(a)
            .or_insert_with(HashSet::new)
            .insert(b);
    }
    
    fn find_root(&self, var: &LifetimeVar) -> LifetimeVar {
        let mut current = var.clone();
        while let Some(parent) = self.equiv_classes.get(&current) {
            if parent == &current {
                break;
            }
            current = parent.clone();
        }
        current
    }
    
    fn validate(&self) -> Result<(), String> {
        // Check for circular within constraints
        for (inner, outers) in &self.within_relations {
            for outer in outers {
                if self.is_within(outer, inner) {
                    return Err(format!(
                        "Circular lifetime constraint: {:?} within {:?} and {:?} within {:?}",
                        inner, outer, outer, inner
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    fn get_all_lifetimes(&self) -> HashMap<LifetimeVar, LifetimeVar> {
        let mut result = HashMap::new();
        
        // Collect all lifetime variables
        let mut all_vars = HashSet::new();
        all_vars.extend(self.equiv_classes.keys().cloned());
        all_vars.extend(self.equiv_classes.values().cloned());
        all_vars.extend(self.within_relations.keys().cloned());
        all_vars.extend(self.outlives_relations.keys().cloned());
        
        // Map each variable to its canonical representative
        for var in all_vars {
            result.insert(var.clone(), self.find_root(&var));
        }
        
        result
    }
}

/// Result of lifetime inference
#[derive(Debug)]
pub struct LifetimeAnnotations {
    /// Mapping from expression IDs to their inferred lifetimes
    pub expr_lifetimes: HashMap<ExprId, LifetimeVar>,
    /// All inferred lifetime variables and their canonical forms
    pub inferred_lifetimes: HashMap<LifetimeVar, LifetimeVar>,
}