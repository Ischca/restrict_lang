# Restrict Language API Documentation

## Compiler API

### Rust Library Usage

```rust
use restrict_lang::{parse_program, TypeChecker, generate};

// Parse source code into AST
let source = "fun main = { val x = 42; x }";
let (remaining, ast) = parse_program(source)
    .map_err(|e| format!("Parse error: {:?}", e))?;

// Type check the AST
let mut type_checker = TypeChecker::new();
type_checker.check_program(&ast)
    .map_err(|e| format!("Type error: {}", e))?;

// Generate WebAssembly code
let wat_code = generate(&ast)
    .map_err(|e| format!("Codegen error: {}", e))?;

println!("Generated WAT:\n{}", wat_code);
```

### Command Line Interface

```bash
# Compile a program
restrict_lang input.rl [output.wat]

# Type check only
restrict_lang --check input.rl

# Show AST
restrict_lang --ast input.rl

# Show tokens
restrict_lang --tokens input.rl

# Help
restrict_lang --help
```

## Core Modules

### Lexer (`src/lexer.rs`)

#### `lex_tokens(input: &str) -> Result<Vec<Token>, String>`

Tokenizes source code into a vector of tokens.

```rust
use restrict_lang::lexer::{lex_tokens, Token};

let tokens = lex_tokens("val x = 42")?;
assert_eq!(tokens, vec![
    Token::Val,
    Token::Ident("x".to_string()),
    Token::Assign,
    Token::IntLit(42)
]);
```

#### Token Types

```rust
pub enum Token {
    // Keywords
    Record, Clone, Freeze, Impl, Context, With,
    Fun, Val, Mut, Then, Else, While, Match,
    Async, Return, True, False, Unit, Some, None,
    
    // Identifiers and Literals
    Ident(String),
    IntLit(i32),
    FloatLit(f64),
    StringLit(String),
    CharLit(char),
    
    // Operators
    Pipe,           // |>
    PipeMut,        // |>>
    Bar,            // |
    Assign,         // =
    Arrow,          // =>
    Plus, Minus, Star, Slash, Percent,
    Eq, Ne, Lt, Le, Gt, Ge,
    
    // Delimiters
    LBrace, RBrace,         // { }
    LParen, RParen,         // ( )
    LBracket, RBracket,     // [ ]
    LArrayBracket, RArrayBracket,  // [| |]
    Comma, Colon, Dot, Semicolon,
    
    Eof,
}
```

### Parser (`src/parser.rs`)

#### `parse_program(input: &str) -> ParseResult<Program>`

Parses source code into an Abstract Syntax Tree.

```rust
use restrict_lang::parser::{parse_program, Program};

let (remaining, program) = parse_program(r#"
    fun add = x:Int, y:Int { x + y }
    val result = (5, 10) add
"#)?;

println!("AST: {:#?}", program);
```

#### AST Node Types

```rust
pub struct Program {
    pub declarations: Vec<Declaration>,
}

pub enum Declaration {
    Function(FunDecl),
    Binding(BindDecl),
    Record(RecordDecl),
    Impl(ImplDecl),
    Context(ContextDecl),
}

pub struct FunDecl {
    pub name: String,
    pub params: Vec<FunParam>,
    pub return_type: Option<Type>,
    pub body: BlockExpr,
}

pub enum Expr {
    IntLit(i32),
    FloatLit(f64),
    StringLit(String),
    CharLit(char),
    BoolLit(bool),
    Unit,
    Ident(String),
    RecordLit(RecordLit),
    Clone(CloneExpr),
    Freeze(Box<Expr>),
    FieldAccess(Box<Expr>, String),
    Call(CallExpr),
    Block(BlockExpr),
    Binary(BinaryExpr),
    Pipe(PipeExpr),
    With(WithExpr),
    Then(ThenExpr),
    While(WhileExpr),
    Match(MatchExpr),
    ListLit(Vec<Box<Expr>>),
    ArrayLit(Vec<Box<Expr>>),
    Some(Box<Expr>),
    None,
    Lambda(LambdaExpr),
}

pub struct LambdaExpr {
    pub params: Vec<String>,
    pub body: Box<Expr>,
}
```

### Type Checker (`src/type_checker.rs`)

#### `TypeChecker::new() -> TypeChecker`

Creates a new type checker instance.

#### `check_program(&mut self, program: &Program) -> Result<(), TypeError>`

Type checks a program AST.

```rust
use restrict_lang::{TypeChecker, TypeError};

let mut checker = TypeChecker::new();
match checker.check_program(&program) {
    Ok(()) => println!("Type checking passed!"),
    Err(TypeError::TypeMismatch { expected, found }) => {
        println!("Type mismatch: expected {}, found {}", expected, found);
    }
    Err(TypeError::UndefinedVariable(var)) => {
        println!("Undefined variable: {}", var);
    }
    Err(TypeError::AffineViolation(var)) => {
        println!("Variable {} used more than once", var);
    }
    // ... handle other errors
}
```

#### Type System

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum TypedType {
    Int32,
    Float64,
    Boolean,
    String,
    Char,
    Unit,
    Record { name: String, frozen: bool },
    Function { params: Vec<TypedType>, return_type: Box<TypedType> },
    Option(Box<TypedType>),
    List(Box<TypedType>),
    Array(Box<TypedType>, usize),
}

#[derive(Debug, Error, PartialEq)]
pub enum TypeError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    
    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },
    
    #[error("Variable {0} has already been used (affine type violation)")]
    AffineViolation(String),
    
    #[error("Cannot reassign to immutable variable {0}")]
    ImmutableReassignment(String),
    
    #[error("Wrong number of arguments: expected {expected}, found {found}")]
    ArityMismatch { expected: usize, found: usize },
    
    // ... other error types
}
```

### Code Generator (`src/codegen.rs`)

#### `generate(program: &Program) -> Result<String, CodeGenError>`

Generates WebAssembly Text Format from a typed AST.

```rust
use restrict_lang::{generate, CodeGenError};

let wat_code = generate(&program)?;
println!("Generated WAT:\n{}", wat_code);

// Write to file
std::fs::write("output.wat", wat_code)?;
```

#### `WasmCodeGen`

The main code generation struct that manages WebAssembly output.

```rust
pub struct WasmCodeGen {
    // Variable to local index mapping
    locals: Vec<HashMap<String, u32>>,
    
    // Function signatures
    functions: HashMap<String, FunctionSig>,
    
    // String constants
    strings: Vec<String>,
    string_offsets: HashMap<String, u32>,
    
    // Memory management
    next_mem_offset: u32,
    arena_stack: Vec<u32>,
    
    // Lambda support
    lambda_counter: u32,
    lambda_functions: Vec<String>,
    function_table: Vec<String>,
    
    // Generated output
    output: String,
}
```

#### Code Generation Errors

```rust
#[derive(Debug, Error)]
pub enum CodeGenError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    
    #[error("Undefined function: {0}")]
    UndefinedFunction(String),
    
    #[error("Type not supported in WASM: {0}")]
    UnsupportedType(String),
    
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}
```

## Standard Library (Planned)

### List Operations

```rust
// Length
fun len = list:List<T> -> Int

// Map operation
fun map = list:List<T>, f:T->U -> List<U>

// Filter operation  
fun filter = list:List<T>, predicate:T->Boolean -> List<T>

// Fold (reduce) operation
fun fold = list:List<T>, initial:U, f:(U,T)->U -> U

// Take first N elements
fun take = list:List<T>, n:Int -> List<T>

// Drop first N elements
fun drop = list:List<T>, n:Int -> List<T>

// Concatenate lists
fun concat = a:List<T>, b:List<T> -> List<T>

// Find first element matching predicate
fun find = list:List<T>, predicate:T->Boolean -> Option<T>

// Check if all elements match predicate
fun all = list:List<T>, predicate:T->Boolean -> Boolean

// Check if any element matches predicate
fun any = list:List<T>, predicate:T->Boolean -> Boolean
```

### Option Operations

```rust
// Map over Option
fun map_option = opt:Option<T>, f:T->U -> Option<U>

// Unwrap with default
fun unwrap_or = opt:Option<T>, default:T -> T

// Chain Option operations
fun and_then = opt:Option<T>, f:T->Option<U> -> Option<U>

// Filter Option
fun filter_option = opt:Option<T>, predicate:T->Boolean -> Option<T>

// Check if Option is Some
fun is_some = opt:Option<T> -> Boolean

// Check if Option is None
fun is_none = opt:Option<T> -> Boolean
```

### String Operations

```rust
// String length
fun str_len = s:String -> Int

// Concatenate strings
fun str_concat = a:String, b:String -> String

// Split string by delimiter
fun str_split = s:String, delimiter:String -> List<String>

// Trim whitespace
fun str_trim = s:String -> String

// Convert to lowercase
fun str_to_lower = s:String -> String

// Convert to uppercase
fun str_to_upper = s:String -> String

// Check if string starts with prefix
fun str_starts_with = s:String, prefix:String -> Boolean

// Check if string ends with suffix
fun str_ends_with = s:String, suffix:String -> Boolean
```

### Arena Management

```rust
// Create new arena with size in bytes
fun new_arena = size:Int -> Arena

// Use default arena for allocations
fun use_default_arena = size:Int -> Unit

// Get current arena usage
fun arena_usage = arena:Arena -> Int

// Get arena capacity
fun arena_capacity = arena:Arena -> Int
```

### Math Operations

```rust
// Mathematical functions
fun abs = x:Int -> Int
fun min = x:Int, y:Int -> Int  
fun max = x:Int, y:Int -> Int
fun pow = base:Int, exp:Int -> Int

// Float operations
fun sqrt = x:Float64 -> Float64
fun sin = x:Float64 -> Float64
fun cos = x:Float64 -> Float64
fun tan = x:Float64 -> Float64
fun floor = x:Float64 -> Float64
fun ceil = x:Float64 -> Float64
fun round = x:Float64 -> Float64
```

## WebAssembly Runtime Interface

### Memory Layout

```wat
;; Memory sections
(memory 1)  ;; 64KB pages

;; Global variables for arena management
(global $arena_start (mut i32) (i32.const 32768))
(global $arena_end (mut i32) (i32.const 65536))
(global $next_alloc (mut i32) (i32.const 32768))

;; String constants section (starts at offset 1024)
(data (i32.const 1024) "Hello World")
```

### Function Calling Convention

```wat
;; Regular function
(func $add (param $x i32) (param $y i32) (result i32)
  local.get $x
  local.get $y
  i32.add)

;; Lambda function with closure
(func $lambda_0 (param $closure i32) (param $x i32) (result i32)
  ;; Load captured variable from closure
  local.get $closure
  i32.const 4
  i32.add
  i32.load
  
  ;; Use parameter
  local.get $x
  i32.add)
```

### WASI Integration

```wat
;; WASI imports for I/O
(import "wasi_snapshot_preview1" "fd_write" 
  (func $fd_write (param i32 i32 i32 i32) (result i32)))
(import "wasi_snapshot_preview1" "proc_exit" 
  (func $proc_exit (param i32)))

;; Print function
(func $print_int (param $value i32)
  ;; Implementation using fd_write
  ...)
```

## Testing API

### Unit Testing Framework

```rust
use restrict_lang::{parse_program, TypeChecker, generate};

fn compile_and_test(source: &str) -> Result<String, String> {
    // Parse
    let (_, ast) = parse_program(source)
        .map_err(|e| format!("Parse error: {:?}", e))?;
    
    // Type check
    let mut type_checker = TypeChecker::new();
    type_checker.check_program(&ast)
        .map_err(|e| format!("Type error: {}", e))?;
    
    // Generate code
    generate(&ast)
        .map_err(|e| format!("Codegen error: {}", e))
}

#[test]
fn test_simple_function() {
    let source = r#"
        fun double = x:Int { x * 2 }
        val result = (21) double
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("func $double"));
    assert!(wat.contains("i32.const 2"));
    assert!(wat.contains("i32.mul"));
}
```

### Integration Testing

```rust
// Test lambda expressions
#[test]
fn test_lambda_closure() {
    let source = r#"
        fun make_adder = n:Int {
            |x| x + n
        }
        val add5 = make_adder(5)
        val result = (10) add5
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("func $lambda_"));
    assert!(wat.contains("param $closure"));
    assert!(wat.contains("call_indirect"));
}

// Test pattern matching
#[test] 
fn test_pattern_matching() {
    let source = r#"
        val numbers = [1, 2, 3]
        val result = numbers match {
            [] => { 0 }
            [head | tail] => { head }
        }
    "#;
    
    let wat = compile_and_test(source).unwrap();
    assert!(wat.contains("br_table"));  // Switch for pattern matching
}
```

## Error Handling

### Common Error Patterns

```rust
// Parse errors
match parse_program(source) {
    Ok((_, ast)) => { /* success */ }
    Err(nom::Err::Error(e)) => {
        println!("Parse error at: {}", e.input);
    }
    Err(nom::Err::Failure(e)) => {
        println!("Parse failure: {}", e.input);
    }
    Err(nom::Err::Incomplete(_)) => {
        println!("Incomplete input");
    }
}

// Type errors
match type_checker.check_program(&ast) {
    Err(TypeError::TypeMismatch { expected, found }) => {
        println!("Expected {} but found {}", expected, found);
    }
    Err(TypeError::AffineViolation(var)) => {
        println!("Variable '{}' used more than once", var);
    }
    Err(TypeError::UndefinedVariable(var)) => {
        println!("Variable '{}' not found", var);
    }
    // ... handle other error types
}
```

### Error Recovery

The parser supports limited error recovery for better developer experience:

```rust
// Parser will attempt to recover from syntax errors
// and continue parsing to find more errors
let errors = parse_with_recovery(source);
for error in errors {
    println!("Line {}: {}", error.line, error.message);
}
```

---

This API documentation covers the main interfaces for working with Restrict Language. For implementation details, see the source code and inline documentation.