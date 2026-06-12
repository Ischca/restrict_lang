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
/// import release.{println}
///
/// fun main: () -> () = {
///     "Hello, World!" |> println
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
/// import release.{public_score}
/// import release.*
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
/// pub fun public_function: () -> Int32 = { 1 }
/// pub record PublicType { value: Int32 }
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
///     x: Float64,
///     y: Float64
/// }
///
/// record Config {
///     host: String,
///     port: Int32
/// }
///
/// // Record with temporal parameters
/// record File<~f> {
///     handle: FileHandle
/// }
///
/// // Record with temporal constraints
/// record Transaction<~tx, ~db> where ~tx within ~db {
///     db: Database<~db>
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RecordDecl {
    /// Name of the record type
    pub name: String,
    /// Type parameters (including temporal parameters)
    pub type_params: Vec<TypeParam>,
    /// Temporal constraints (e.g., ~tx within ~db)
    pub temporal_constraints: Vec<TemporalConstraint>,
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
///     fun distance: (self: Point, other: Point) -> Float64 = {
///         val dx = self.x - other.x
///         val dy = self.y - other.y
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
///     to_string: String
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
/// fun greet: (name: String) -> String = {
///     "Hello, " + name + "!"
/// }
///
/// // Generic function
/// fun identity: <T>(value: T) -> T = {
///     value
/// }
///
/// // With type bounds
/// fun display: <T: ToString>(value: T) -> String = {
///     value |> toString
/// }
///
/// // With temporal parameters
/// fun read_file: <~io>(file: File<~io>) -> String = {
///     file |> read
/// }
///
/// // With temporal constraints
/// fun begin_tx: <~db, ~tx>(db: Database<~db>) -> Transaction<~tx, ~db> where ~tx within ~db = {
///     db |> begin
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FunDecl {
    /// Function name
    pub name: String,
    /// Whether this is an async function
    pub is_async: bool,
    /// Generic type parameters with bounds: `<T: Display, U: Clone>`
    pub type_params: Vec<TypeParam>,
    /// Temporal constraints (e.g., ~tx within ~db)
    pub temporal_constraints: Vec<TemporalConstraint>,
    /// Function parameters
    pub params: Vec<Param>,
    /// Optional explicit return type annotation
    pub return_type: Option<Type>,
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
/// <~t>             // Temporal type parameter
/// <~tx, ~db> where ~tx within ~db  // Temporal with constraints
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParam {
    /// Parameter name (e.g., "T")
    pub name: String,
    /// Type constraints (e.g., `T: Display + Clone`)
    pub bounds: Vec<TypeBound>,
    /// Derivation bound (e.g., `T from ParentType`)
    pub derivation_bound: Option<String>,
    /// Whether this is a temporal type parameter (starts with ~)
    pub is_temporal: bool,
}

/// Type bound constraint for generic parameters.
///
/// # Example
///
/// ```restrict
/// fun process: <T: Display + Clone>(value: T) -> T = { value }
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
/// fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
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

/// Binding declaration (val statement).
///
/// # Examples
///
/// ```restrict
/// val x = 42              // Immutable binding
/// val pi: Float64 = 3.14  // Explicit type annotation
/// mut val count = 0       // Mutable binding
/// val Point { x, y } = point
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BindDecl {
    /// Whether the binding is mutable
    pub mutable: bool,
    /// Binding pattern (can be simple name or complex pattern)
    pub pattern: Pattern,
    /// Optional explicit type annotation for the binding
    pub type_annotation: Option<Type>,
    /// Initial value
    pub value: Box<Expr>,
}

/// Stable identity of an expression node within one `Program`.
///
/// Ids are assigned as a dense pre-order numbering over the program
/// structure, so they do not depend on allocation addresses. Cloning an
/// expression keeps its id: facts recorded for a node therefore stay
/// valid for clones of the same numbered program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    /// Placeholder for nodes that have not been numbered yet: parser
    /// output before numbering and nodes synthesized during desugaring.
    pub const DUMMY: NodeId = NodeId(u32::MAX);
}

/// Expression node: a stable id plus the expression variant.
///
/// Node ids are identity metadata, not structure. `PartialEq` therefore
/// compares only `kind`, so structural AST comparisons (e.g. parser
/// tests) are independent of numbering state.
#[derive(Debug, Clone)]
pub struct Expr {
    /// Stable node id (`NodeId::DUMMY` until numbering)
    pub id: NodeId,
    /// The expression variant
    pub kind: ExprKind,
}

impl Expr {
    /// Construct an unnumbered expression node.
    pub fn new(kind: ExprKind) -> Self {
        Expr {
            id: NodeId::DUMMY,
            kind,
        }
    }
}

impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}

/// Expression variants in the AST.
///
/// Expressions are the core computational elements of Restrict Language.
/// They follow the affine type system where each value can be used at most once.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    // Literals
    /// Integer literal (e.g., `42`, `0xFF`, `0b1010`)
    IntLit(i64),
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
    /// Unary operators (`-`, `!`)
    Unary(UnaryExpr),
    /// Explicit cast (`expr as Type`)
    Cast(CastExpr),

    // Pipe operations
    /// Pipe operator (`|>`)
    Pipe(PipeExpr),

    // Context operations
    /// With expression for resource management
    With(WithExpr),

    // Lifetime scope
    /// With lifetime expression for temporal scope management
    WithLifetime(WithLifetimeExpr),

    // Block
    /// Block expression containing multiple statements
    Block(BlockExpr),

    // Field access
    /// Field access (e.g., `point.x`)
    FieldAccess(Box<Expr>, String),

    // List literal
    /// List literal (e.g., `[1, 2, 3]`)
    ListLit(Vec<Box<Expr>>),

    /// Range literal (e.g., `[1..10]`)
    RangeLit(RangeLit),

    // Array literal
    /// Array literal with fixed size
    ArrayLit(Vec<Box<Expr>>),

    // Option constructors
    /// Some variant of Option type
    Some(Box<Expr>),
    /// None variant of Option type
    None,
    /// Ok variant of Result type
    Ok(Box<Expr>),
    /// Err variant of Result type
    Err(Box<Expr>),

    // Lambda expression
    /// Anonymous function (e.g., `|x| x + 1`)
    Lambda(LambdaExpr),

    // Async operations
    /// Await expression (e.g., `future await`)
    Await(Box<Expr>),
    /// Spawn expression for planned async support
    Spawn(Box<Expr>),
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
///
/// Can be either a regular field assignment (`field: value`) or a spread expression (`...expr`).
#[derive(Debug, Clone, PartialEq)]
pub enum FieldInit {
    /// Regular field assignment: `field: value`
    Field {
        /// Field name
        name: String,
        /// Field value expression
        value: Box<Expr>,
    },
    /// Spread expression: `...expr`
    Spread(Box<Expr>),
}

/// Range literal with integer endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct RangeLit {
    /// Start endpoint expression
    pub start: Box<Expr>,
    /// End endpoint expression
    pub end: Box<Expr>,
}

/// Clone expression with field updates.
///
/// # Example
///
/// ```restrict
/// val moved = point.clone { x: 30.0 }
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
/// val dog = animal_proto.clone { species: "dog", sound: "woof" }
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

/// Then/else expression.
///
/// # Example
///
/// ```restrict
/// x > 0 then {
///     "positive"
/// } else x < 0 then {
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
/// count > 0 while {
///     count |> println
///     count = count - 1
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
/// result match {
///     Ok(value) => { value |> process }
///     Err(error) => { error |> handle_error }
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
    /// Record destructuring with spread `{ field1, field2, ...rest }`
    RecordDestruct {
        /// Record type name
        type_name: String,
        /// Fields to extract
        fields: Vec<(String, Pattern)>,
        /// Optional residual binding (the ...rest part)
        rest: Option<String>,
    },
    /// Some variant pattern
    Some(Box<Pattern>),
    /// None variant pattern
    None,
    /// Ok variant pattern
    Ok(Box<Pattern>),
    /// Err variant pattern
    Err(Box<Pattern>),
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
    Int(i64),
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
/// (1, 2) add
/// (list, |x| x * 2) map
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

/// Unary operation expression.
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryExpr {
    /// Unary operator
    pub op: UnaryOp,
    /// Operand
    pub expr: Box<Expr>,
}

/// Explicit cast expression.
#[derive(Debug, Clone, PartialEq)]
pub struct CastExpr {
    /// Expression being cast
    pub expr: Box<Expr>,
    /// Target type
    pub target: Type,
}

/// Unary operators.
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    /// Numeric negation `-`
    Neg,
    /// Logical negation `!`
    Not,
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
    /// Logical and `&&`
    And,
    /// Logical or `||`
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipeExpr {
    pub expr: Box<Expr>,
    pub op: PipeOp,
    pub target: PipeTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipeOp {
    Pipe, // |>
    Bar,  // |
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipeTarget {
    Ident(String),   // For binding
    Expr(Box<Expr>), // For function application
}

#[derive(Debug, Clone, PartialEq)]
pub struct WithExpr {
    /// Context name (e.g., "Database" in "with Database { ... }")
    pub context_name: String,
    /// Field bindings for the context (e.g., "connection: conn")
    pub bindings: Vec<FieldInit>,
    /// Body of the with expression
    pub body: BlockExpr,
}

/// With lifetime expression for temporal scope management.
///
/// # Example
///
/// ```restrict
/// with lifetime<~f> {
///     val file = File { path: "data.txt" }
///     file |> read
/// }  // file automatically cleaned up
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct WithLifetimeExpr {
    /// Lifetime parameter (e.g., "f" for ~f)
    pub lifetime: String,
    /// Anonymous lifetime if no name provided
    pub anonymous: bool,
    /// Temporal constraints for this lifetime
    pub constraints: Vec<TemporalConstraint>,
    /// Body of the lifetime scope
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
    pub params: Vec<LambdaParam>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    pub name: String,
    pub type_annotation: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(String),
    Generic(String, Vec<Type>),
    Function(Vec<Type>, Box<Type>), // (param_types, return_type)
    Temporal(String, Vec<String>),  // Type with temporal parameters (e.g., File<~f>)
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
            Type::Temporal(name, temporals) => {
                write!(f, "{}<", name)?;
                for (i, temporal) in temporals.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "~{}", temporal)?;
                }
                write!(f, ">")
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
            BinaryOp::And => write!(f, "&&"),
            BinaryOp::Or => write!(f, "||"),
        }
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
        }
    }
}

/// Temporal constraint expressing relationships between temporal variables.
///
/// # Example
///
/// ```restrict
/// where ~tx within ~db    // Transaction lifetime within database lifetime
/// where ~a within ~b      // General containment relationship
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalConstraint {
    /// The inner temporal variable (e.g., ~tx)
    pub inner: String,
    /// The outer temporal variable (e.g., ~db)
    pub outer: String,
}
