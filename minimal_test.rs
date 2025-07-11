use restrict_lang::parse_program;

fn main() {
    // Test the exact boundary
    let tests = vec![
        ("Working: 49 chars", "fun f = { val x = [] (x) match { [a] => { 0 } } }"),
        ("Working: 50 chars", "fun f = { val x = [] (x) match { [ab] => { 0 } } }"),
        ("Working: 51 chars", "fun f = { val x = [] (x) match { [abc] => { 0 } } }"),
        ("Failing: 52 chars", "fun f = { val x = [] (x) match { [a, b] => { 0 } } }"),
        ("Failing: 53 chars", "fun f = { val x = [] (x) match { [a, bc] => { 0 } } }"),
        ("Different pattern", "fun f = { val x = [] (x) match { [] => { 0 } [a, b] => { 1 } } }"),
    ];
    
    for (name, input) in tests {
        println!("\n{} (length {}):", name, input.len());
        match parse_program(input) {
            Ok((rem, _prog)) => {
                println!("✓ Success! {} chars remaining", rem.len());
            }
            Err(e) => {
                println!("✗ Failed: {:?}", e);
                match e {
                    nom::Err::Error(ref err) | nom::Err::Failure(ref err) => {
                        let pos = input.len() - err.input.len();
                        println!("  Failed at position {}", pos);
                        println!("  Remaining input: {:?}", &err.input[..20.min(err.input.len())]);
                    }
                    _ => {}
                }
            }
        }
    }
}