pub mod lexer;
pub mod ast;
pub mod parser;
pub mod type_checker;
pub mod codegen;
pub mod test_framework;
pub mod debug_visualizer;
pub mod dev_tools;
pub mod web;

pub use lexer::*;
pub use ast::*;
pub use parser::*;
pub use type_checker::*;
pub use codegen::*;