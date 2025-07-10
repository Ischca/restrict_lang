use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<TopDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopDecl {
    Record(RecordDecl),
    Impl(ImplBlock),
    Context(ContextDecl),
    Function(FunDecl),
    Binding(BindDecl),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImplBlock {
    pub target: String,
    pub functions: Vec<FunDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContextDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub body: BlockExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub context_bound: Option<String>, // For @Context parameters
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindDecl {
    pub mutable: bool,
    pub name: String,
    pub value: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // Literals
    IntLit(i32),
    FloatLit(f64),
    StringLit(String),
    CharLit(char),
    BoolLit(bool),
    Unit,
    
    // Identifiers
    Ident(String),
    
    // Record operations
    RecordLit(RecordLit),
    Clone(CloneExpr),
    Freeze(Box<Expr>),
    
    // Control flow
    Then(ThenExpr),
    While(WhileExpr),
    Match(MatchExpr),
    
    // Function call
    Call(CallExpr),
    
    // Binary operations
    Binary(BinaryExpr),
    
    // Pipe operations
    Pipe(PipeExpr),
    
    // Context operations
    With(WithExpr),
    
    // Block
    Block(BlockExpr),
    
    // Field access
    FieldAccess(Box<Expr>, String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordLit {
    pub name: String,
    pub fields: Vec<FieldInit>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    pub name: String,
    pub value: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloneExpr {
    pub base: Box<Expr>,
    pub updates: RecordLit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThenExpr {
    pub condition: Box<Expr>,
    pub then_block: BlockExpr,
    pub else_ifs: Vec<(Box<Expr>, BlockExpr)>,
    pub else_block: Option<BlockExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileExpr {
    pub condition: Box<Expr>,
    pub body: BlockExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub expr: Box<Expr>,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: BlockExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,
    Literal(Literal),
    Ident(String),
    Record(String, Vec<(String, Pattern)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i32),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Unit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub function: Box<Expr>,
    pub args: Vec<Box<Expr>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: BinaryOp,
    pub right: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
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
    Expr(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(String),
    Generic(String, Vec<Type>),
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