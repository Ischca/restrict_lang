use restrict_lang::lex_token;

fn main() {
    let input = "record Enemy { hp: Int }";
    println!("Trying to lex: {:?}", input);
    
    let mut remaining = input;
    while !remaining.is_empty() {
        match lex_token(remaining) {
            Ok((rest, token)) => {
                println!("Token: {:?}", token);
                remaining = rest;
            }
            Err(e) => {
                println!("Error at: {:?}", remaining);
                println!("Error: {:?}", e);
                break;
            }
        }
    }
}