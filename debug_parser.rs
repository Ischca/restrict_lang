use restrict_lang::parse_program;

fn main() {
    // 問題のある入力
    let input = "fun main = { with Arena { val lst = [1, 2, 3] val result = (lst) match { [] => { 0 } [a] => { a } [a, b] => { a + b } [a, b, c] => { a + b + c } _ => { -1 } } result } }";
    
    println!("Input length: {}", input.len());
    println!("Input: {:?}", input);
    
    // パースを試みる
    match parse_program(input) {
        Ok((remaining, ast)) => {
            println!("\n✓ Parse successful!");
            println!("Remaining input length: {}", remaining.len());
            println!("Remaining: {:?}", remaining);
            println!("Declarations: {}", ast.declarations.len());
        }
        Err(e) => {
            println!("\n✗ Parse failed!");
            println!("Error: {:?}", e);
        }
    }
}