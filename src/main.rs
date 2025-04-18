use restrict_lang::{interpret, parse};

fn main() {
    // let input = "let add = fun x -> fun y -> x + y in add 1 2";
    // read test file
    println!("Starting main function");
    let input = std::fs::read_to_string("./test.rl").unwrap();
    // let ast = parse(input).unwrap();
    // match interpret(&ast) {
    //     Ok(result) => println!("Result: {:?}", result),
    //     Err(e) => println!("Error: {}", e),
    // }
    match parse(input.as_str()) {
        Ok(expr) => {
            // let mut env = HashMap::new();
            // println!("Expr: {:?}", &expr);
            match interpret(&expr) {
                Ok(result) => println!("Result: {:?}", result),
                Err(e) => eprintln!("Evaluation error: {}", e),
            }
        }
        Err(e) => eprintln!("Parse error: {}", e),
    }
}
