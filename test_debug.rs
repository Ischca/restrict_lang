use restrict_lang::{parse_program};

fn main() {
    let source = r#"
        fun main = {
            val a = 100;
            val b = 50;
            val result = a - b;
            result
        }
    "#;
    
    let result = parse_program(source);
    println!("Parse result: {:#?}", result);
}