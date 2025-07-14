//! # Abstract Syntax Tree (AST)
//!
//! This module defines the AST nodes for Restrict Language programs.
//! The AST represents the syntactic structure of parsed source code
//! and serves as the input for type checking and code generation.
//!
//! ## Design Principles
//!
//! - **Affine-aware**: The AST is designed with affine types in mind
//! - **OSV-friendly**: Structures support the Object-Subject-Verb syntax
//! - **Generic-ready**: Support for type parameters and constraints
//! - **Pattern-rich**: Comprehensive pattern matching support

use std::fmt;

/// The root node of a Restrict Language program.
/// 
/// A program consists of import declarations followed by top-level declarations.
/// 
/// # Example
/// 
/// ```restrict
/// import std.io.{println, readLine};
/// import std.collections.*;
/// 
/// fn main() {
///     "Hello, World!" |> println;
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Import declarations at the beginning of the file
    pub imports: Vec<ImportDecl>,
    /// Top-level declarations (functions, types, etc.)
    pub declarations: Vec<TopDecl>,
}

/// An import declaration bringing external items into scope.
/// 
/// # Examples
/// 
/// ```restrict
/// import std.io.*;                    // Import all
/// import std.collections.{Vec, Map};  // Import specific items
/// import warder.config;               // Import module
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ImportDecl {
    /// The module path as a sequence of identifiers
    pub module_path: Vec<String>,
    /// What to import from the module
    pub items: ImportItems,
}

/// Specifies which items to import from a module.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportItems {
    /// Import all public items (`import module.*`)
    All,
    /// Import specific named items (`import module.{foo, bar}`)
    Named(Vec<String>),
}

/// Top-level declarations that can appear in a program.
/// 
/// These form the main structure of a Restrict Language module.
#[derive(Debug, Clone, PartialEq)]
pub enum TopDecl {
    /// Record type declaration (struct-like)
    Record(RecordDecl),
    /// Implementation block for a type
    Impl(ImplBlock),
    /// Context declaration (type class)
    Context(ContextDecl),
    /// Function declaration
    Function(FunDecl),
    /// Global binding declaration
    Binding(BindDecl),
    /// Export declaration (makes item public)
    Export(ExportDecl),
}

/// Export declaration that makes an item publicly available.
/// 
/// # Example
/// 
/// ```restrict
/// export fn publicFunction() { ... }
/// export record PublicType { ... }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ExportDecl {
    /// The declaration being exported
    pub item: Box<TopDecl>,
}

/// Record type declaration (similar to struct in other languages).
/// 
/// Records in Restrict Language support prototype-based inheritance
/// through `clone` and `freeze` operations.
/// 
/// # Example
/// 
/// ```restrict
/// record Point {
///     x: f64,
///     y: f64
/// }
/// 
/// // Frozen record (immutable prototype)
/// frozen record Config {
///     host: String,
///     port: i32
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RecordDecl {
    /// Name of the record type
    pub name: String,
    /// Fields in the record
    pub fields: Vec<FieldDecl>,
    /// Whether this record is frozen (immutable prototype)
    pub frozen: bool,
    /// Whether this record is sealed (no further fields can be added)
    pub sealed: bool,
    /// Hash of parent prototype if this was created via clone
    pub parent_hash: Option<String>,
}

/// Field declaration within a record.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    /// Field name
    pub name: String,
    /// Field type
    pub ty: Type,
}

/// Implementation block that adds methods to a type.
/// 
/// # Example
/// 
/// ```restrict
/// impl Point {
///     fn distance(self, other: Point) -> f64 {
///         let dx = self.x - other.x;
///         let dy = self.y - other.y;
///         (dx * dx + dy * dy) |> sqrt
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    /// The type being implemented for
    pub target: String,
    /// Methods in the implementation
    pub functions: Vec<FunDecl>,
}

/// Context declaration (similar to type classes or traits).
/// 
/// Contexts define interfaces that types can implement.
/// 
/// # Example
/// 
/// ```restrict
/// context Printable {
///     toString: fn() -> String
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ContextDecl {
    /// Name of the context
    pub name: String,
    /// Required fields/methods
    pub fields: Vec<FieldDecl>,
}

/// Function declaration with support for generics and type bounds.
/// 
/// # Example
/// 
/// ```restrict
/// fn greet(name: String) {
///     "Hello, " ++ name ++ "!" |> println;
/// }
/// 
/// // Generic function
/// fn identity<T>(value: T) -> T {
///     value
/// }
/// 
/// // With type bounds
/// fn display<T: ToString>(value: T) {
///     value |> toString |> println;
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FunDecl {
    /// Function name
    pub name: String,
    /// Generic type parameters with bounds: `<T: Display, U: Clone>`
    pub type_params: Vec<TypeParam>,
    /// Function parameters
    pub params: Vec<Param>,
    /// Function body
    pub body: BlockExpr,
}

/// Generic type parameter with optional bounds.
/// 
/// Supports both trait bounds and derivation bounds.
/// 
/// # Examples
/// 
/// ```restrict
/// <T>              // Simple type parameter
/// <T: Clone>       // With trait bound
/// <T: Clone + Display>  // Multiple bounds
/// <T from Animal>  // Derivation bound
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    /// Parameter name (e.g., "T")
    pub name: String,
    /// Type constraints (e.g., `T: Display + Clone`)
    pub bounds: Vec<TypeBound>,
    /// Derivation bound (e.g., `T from ParentType`)
    pub derivation_bound: Option<String>,
}

/// Type bound constraint for generic parameters.
/// 
/// # Example
/// 
/// ```restrict
/// fn process<T: Display + Clone>(value: T) { ... }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TypeBound {
    /// Trait name (e.g., "Display", "Clone", "Debug")
    pub trait_name: String,
}

/// Function parameter with optional context binding.
/// 
/// # Examples
/// 
/// ```restrict
/// fn add(x: i32, y: i32) -> i32 { ... }
/// fn withContext(@logger: Logger, data: String) { ... }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub ty: Type,
    /// Context bound for `@Context` parameters
    pub context_bound: Option<String>,
}

/// Binding declaration (let statement).
/// 
/// # Examples
/// 
/// ```restrict
/// let x = 42;              // Immutable binding
/// let mut count = 0;       // Mutable binding
/// let (a, b) = (1, 2);     // Pattern binding
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BindDecl {
    /// Whether the binding is mutable
    pub mutable: bool,
    /// Variable name
    pub name: String,
    /// Initial value
    pub value: Box<Expr>,
}

/// Expression nodes in the AST.
/// 
/// Expressions are the core computational elements of Restrict Language.
/// They follow the affine type system where each value can be used at most once.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Literals
    /// Integer literal (e.g., `42`, `0xFF`, `0b1010`)
    IntLit(i32),
    /// Floating-point literal (e.g., `3.14`, `2.5e-10`)
    FloatLit(f64),
    /// String literal (e.g., `"hello"`, `r"raw string"`)
    StringLit(String),
    /// Character literal (e.g., `'a'`, `'\n'`)
    CharLit(char),
    /// Boolean literal (`true` or `false`)
    BoolLit(bool),
    /// Unit value `()`
    Unit,
    
    // Identifiers
    /// Variable or function reference
    Ident(String),
    
    // Record operations
    /// Record literal construction
    RecordLit(RecordLit),
    /// Clone expression (`clone expr`)
    Clone(CloneExpr),
    /// Freeze expression (`freeze expr`)
    Freeze(Box<Expr>),
    /// Prototype cloning with modifications
    PrototypeClone(PrototypeCloneExpr),
    
    // Control flow
    /// If-then-else expression
    Then(ThenExpr),
    /// While loop
    While(WhileExpr),
    /// Pattern matching
    Match(MatchExpr),
    
    // Function call
    /// Function application
    Call(CallExpr),
    
    // Binary operations
    /// Binary operators (+, -, *, /, etc.)
    Binary(BinaryExpr),
    
    // Pipe operations
    /// Pipe operator (`|>` and `|>>`)
    Pipe(PipeExpr),
    
    // Context operations
    /// With expression for resource management
    With(WithExpr),
    
    // Block
    /// Block expression containing multiple statements
    Block(BlockExpr),
    
    // Field access
    /// Field access (e.g., `point.x`)
    FieldAccess(Box<Expr>, String),
    
    // List literal
    /// List literal (e.g., `[1, 2, 3]`)
    ListLit(Vec<Box<Expr>>),
    
    // Array literal
    /// Array literal with fixed size
    ArrayLit(Vec<Box<Expr>>),
    
    // Option constructors
    /// Some variant of Option type
    Some(Box<Expr>),
    /// None variant of Option type
    None,
    
    // Lambda expression
    /// Anonymous function (e.g., `|x| x + 1`)
    Lambda(LambdaExpr),
}

/// Record literal for creating record instances.
/// 
/// # Example
/// 
/// ```restrict
/// Point { x: 10.0, y: 20.0 }
/// User { name: "Alice", age: 30 }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RecordLit {
    /// Record type name
    pub name: String,
    /// Field initializers
    pub fields: Vec<FieldInit>,
}

/// Field initialization in a record literal.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    /// Field name
    pub name: String,
    /// Field value expression
    pub value: Box<Expr>,
}

/// Clone expression with field updates.
/// 
/// # Example
/// 
/// ```restrict
/// clone point with { x: 30.0 }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CloneExpr {
    /// Base expression to clone from
    pub base: Box<Expr>,
    /// Field updates to apply
    pub updates: RecordLit,
}

/// Prototype-based cloning with derivation.
/// 
/// Creates a new instance derived from a prototype.
/// 
/// # Example
/// 
/// ```restrict
/// let dog = clone animalProto with {
///     species: "dog",
///     sound: "woof"
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PrototypeCloneExpr {
    /// Name of the prototype to clone
    pub base: String,
    /// Differential updates to apply
    pub updates: RecordLit,
    /// Whether to freeze the result immediately
    pub freeze_immediately: bool,
    /// Whether to seal (prevent further derivation)
    pub sealed: bool,
}

/// If-then-else expression.
/// 
/// # Example
/// 
/// ```restrict
/// if x > 0 {
///     "positive"
/// } else if x < 0 {
///     "negative"
/// } else {
///     "zero"
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ThenExpr {
    /// Condition to test
    pub condition: Box<Expr>,
    /// Block to execute if condition is true
    pub then_block: BlockExpr,
    /// Optional else-if clauses
    pub else_ifs: Vec<(Box<Expr>, BlockExpr)>,
    /// Optional else block
    pub else_block: Option<BlockExpr>,
}

/// While loop expression.
/// 
/// # Example
/// 
/// ```restrict
/// while count > 0 {
///     count |> println;
///     count = count - 1;
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct WhileExpr {
    /// Loop condition
    pub condition: Box<Expr>,
    /// Loop body
    pub body: BlockExpr,
}

/// Pattern matching expression.
/// 
/// # Example
/// 
/// ```restrict
/// match result {
///     Ok(value) => value |> process,
///     Err(error) => error |> handleError
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    /// Expression to match against
    pub expr: Box<Expr>,
    /// Match arms with patterns and bodies
    pub arms: Vec<MatchArm>,
}

/// A single arm in a match expression.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// Pattern to match
    pub pattern: Pattern,
    /// Expression to evaluate if pattern matches
    pub body: BlockExpr,
}

/// Pattern for pattern matching.
/// 
/// Supports literals, destructuring, and list patterns.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Wildcard pattern `_`
    Wildcard,
    /// Literal pattern
    Literal(Literal),
    /// Variable binding pattern
    Ident(String),
    /// Record destructuring pattern
    Record(String, Vec<(String, Pattern)>),
    /// Some variant pattern
    Some(Box<Pattern>),
    /// None variant pattern
    None,
    /// Empty list pattern `[]`
    EmptyList,
    /// List cons pattern `[head | tail]`
    ListCons(Box<Pattern>, Box<Pattern>),
    /// Exact list pattern `[a, b, c]`
    ListExact(Vec<Box<Pattern>>),
}

/// Literal values that can appear in patterns and expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Integer literal
    Int(i32),
    /// Floating-point literal
    Float(f64),
    /// String literal
    String(String),
    /// Character literal
    Char(char),
    /// Boolean literal
    Bool(bool),
    /// Unit literal `()`
    Unit,
}

/// Function call expression.
/// 
/// # Example
/// 
/// ```restrict
/// add(1, 2)
/// map(list, |x| x * 2)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    /// Function to call
    pub function: Box<Expr>,
    /// Arguments to pass
    pub args: Vec<Box<Expr>>,
}

/// Binary operation expression.
/// 
/// # Example
/// 
/// ```restrict
/// x + y
/// a * b
/// name == "Alice"
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    /// Left operand
    pub left: Box<Expr>,
    /// Binary operator
    pub op: BinaryOp,
    /// Right operand
    pub right: Box<Expr>,
}

/// Binary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    /// Addition `+`
    Add,
    /// Subtraction `-`
    Sub,
    /// Multiplication `*`
    Mul,
    /// Division `/`
    Div,
    /// Modulo `%`
    Mod,
    /// Equality `==`
    Eq,
    /// Inequality `!=`
    Ne,
    /// Less than `<`
    Lt,
    /// Less than or equal `<=`
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipeExpr {
    pub expr: Box<Expr>,
    pub op: PipeOp,
    pub target: PipeTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipeOp {
    Pipe,      // |>
    PipeMut,   // |>>
    Bar,       // |
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipeTarget {
    Ident(String),       // For binding
    Expr(Box<Expr>),     // For function application
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithExpr {
    pub contexts: Vec<String>,
    pub body: BlockExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockExpr {
    pub statements: Vec<Stmt>,
    pub expr: Option<Box<Expr>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Binding(BindDecl),
    Assignment(AssignStmt),
    Expr(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub name: String,
    pub value: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LambdaExpr {
    pub params: Vec<String>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(String),
    Generic(String, Vec<Type>),
    Function(Vec<Type>, Box<Type>),  // (param_types, return_type)
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Named(name) => write!(f, "{}", name),
            Type::Generic(name, params) => {
                write!(f, "{}<", name)?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                write!(f, ">")
            }
            Type::Function(params, ret) => {
                write!(f, "(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", ret)
            }
        }
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Sub => write!(f, "-"),
            BinaryOp::Mul => write!(f, "*"),
            BinaryOp::Div => write!(f, "/"),
            BinaryOp::Mod => write!(f, "%"),
            BinaryOp::Eq => write!(f, "=="),
            BinaryOp::Ne => write!(f, "!="),
            BinaryOp::Lt => write!(f, "<"),
            BinaryOp::Le => write!(f, "<="),
            BinaryOp::Gt => write!(f, ">"),
            BinaryOp::Ge => write!(f, ">="),
        }
    }
}