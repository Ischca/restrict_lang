use crate::ast::*;
use crate::lexer::lex_tokens;
use crate::parser::parse_program;
use std::fmt::Write;

pub struct DebugVisualizer {
    output: String,
    indent: usize,
}

impl DebugVisualizer {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }
    
    pub fn visualize_tokens(input: &str) -> String {
        let mut output = String::new();
        writeln!(&mut output, "=== TOKEN STREAM ===").unwrap();
        
        match lex_tokens(input) {
            Ok(tokens) => {
                for (i, token) in tokens.iter().enumerate() {
                    writeln!(&mut output, "{:3}: {:?}", i, token).unwrap();
                }
            }
            Err(e) => {
                writeln!(&mut output, "Lexer error: {:?}", e).unwrap();
            }
        }
        
        output
    }
    
    pub fn visualize_parse_tree(input: &str) -> String {
        let mut viz = DebugVisualizer::new();
        
        writeln!(&mut viz.output, "=== PARSE TREE ===").unwrap();
        match parse_program(input) {
            Ok((remaining, ast)) => {
                if !remaining.trim().is_empty() {
                    writeln!(&mut viz.output, "WARNING: Unparsed input: '{}'", remaining).unwrap();
                }
                viz.visit_program(&ast);
            }
            Err(e) => {
                writeln!(&mut viz.output, "Parse error: {:?}", e).unwrap();
            }
        }
        
        viz.output
    }
    
    fn indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
    }
    
    fn visit_program(&mut self, program: &Program) {
        writeln!(&mut self.output, "Program").unwrap();
        self.indent += 1;
        for decl in &program.declarations {
            self.visit_top_decl(decl);
        }
        self.indent -= 1;
    }
    
    fn visit_top_decl(&mut self, decl: &TopDecl) {
        self.indent();
        match decl {
            TopDecl::Function(f) => {
                writeln!(&mut self.output, "Function: {}", f.name).unwrap();
                self.indent += 1;
                for param in &f.params {
                    self.indent();
                    writeln!(&mut self.output, "Param: {}: {:?}", param.name, param.ty).unwrap();
                }
                self.visit_block(&f.body);
                self.indent -= 1;
            }
            TopDecl::Record(r) => {
                writeln!(&mut self.output, "Record: {}", r.name).unwrap();
                self.indent += 1;
                for field in &r.fields {
                    self.indent();
                    writeln!(&mut self.output, "Field: {}: {:?}", field.name, field.ty).unwrap();
                }
                self.indent -= 1;
            }
            TopDecl::Binding(b) => {
                writeln!(&mut self.output, "Binding: {} (mut: {})", b.name, b.mutable).unwrap();
                self.indent += 1;
                self.visit_expr(&b.value);
                self.indent -= 1;
            }
            _ => writeln!(&mut self.output, "{:?}", decl).unwrap(),
        }
    }
    
    fn visit_expr(&mut self, expr: &Expr) {
        self.indent();
        match expr {
            Expr::IntLit(n) => writeln!(&mut self.output, "IntLit: {}", n).unwrap(),
            Expr::Ident(name) => writeln!(&mut self.output, "Ident: {}", name).unwrap(),
            Expr::Call(call) => {
                writeln!(&mut self.output, "Call:").unwrap();
                self.indent += 1;
                self.indent();
                writeln!(&mut self.output, "Function:").unwrap();
                self.indent += 1;
                self.visit_expr(&call.function);
                self.indent -= 1;
                self.indent();
                writeln!(&mut self.output, "Args:").unwrap();
                self.indent += 1;
                for arg in &call.args {
                    self.visit_expr(arg);
                }
                self.indent -= 2;
            }
            Expr::Block(block) => self.visit_block(block),
            _ => writeln!(&mut self.output, "{:?}", expr).unwrap(),
        }
    }
    
    fn visit_block(&mut self, block: &BlockExpr) {
        self.indent();
        writeln!(&mut self.output, "Block:").unwrap();
        self.indent += 1;
        for stmt in &block.statements {
            self.visit_stmt(stmt);
        }
        if let Some(expr) = &block.expr {
            self.indent();
            writeln!(&mut self.output, "Return:").unwrap();
            self.indent += 1;
            self.visit_expr(expr);
            self.indent -= 1;
        }
        self.indent -= 1;
    }
    
    fn visit_stmt(&mut self, stmt: &Stmt) {
        self.indent();
        match stmt {
            Stmt::Binding(b) => {
                writeln!(&mut self.output, "Let: {} (mut: {})", b.name, b.mutable).unwrap();
                self.indent += 1;
                self.visit_expr(&b.value);
                self.indent -= 1;
            }
            Stmt::Assignment(a) => {
                writeln!(&mut self.output, "Assign: {}", a.name).unwrap();
                self.indent += 1;
                self.visit_expr(&a.value);
                self.indent -= 1;
            }
            Stmt::Expr(e) => {
                writeln!(&mut self.output, "ExprStmt:").unwrap();
                self.indent += 1;
                self.visit_expr(e);
                self.indent -= 1;
            }
        }
    }
}

// Interactive REPL for testing
pub fn run_debug_repl() {
    use std::io::{self, Write};
    
    println!("Restrict Language Debug REPL");
    println!("Commands: :tokens, :parse, :quit");
    
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        if input.starts_with(':') {
            match input {
                ":quit" => break,
                ":tokens" => {
                    print!("Enter code: ");
                    io::stdout().flush().unwrap();
                    let mut code = String::new();
                    io::stdin().read_line(&mut code).unwrap();
                    println!("{}", DebugVisualizer::visualize_tokens(&code));
                }
                ":parse" => {
                    print!("Enter code: ");
                    io::stdout().flush().unwrap();
                    let mut code = String::new();
                    io::stdin().read_line(&mut code).unwrap();
                    println!("{}", DebugVisualizer::visualize_parse_tree(&code));
                }
                _ => println!("Unknown command: {}", input),
            }
        } else {
            // Parse and visualize
            println!("{}", DebugVisualizer::visualize_tokens(input));
            println!("{}", DebugVisualizer::visualize_parse_tree(input));
        }
    }
}