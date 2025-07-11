use restrict_lang::parser::{parse_program, top_decl};

fn main() {
    // Test inputs of varying complexity
    let tests = vec![
        ("Simple working", "fun f = { 1 }"),
        ("List pattern [a]", "fun f = { val x = [] (x) match { [a] => { 0 } } }"),
        ("List pattern [a, b] - FAILS", "fun f = { val x = [] (x) match { [a, b] => { 0 } } }"),
        
        // Test if it's related to the pattern itself or the overall length
        ("Without match", "fun f = { val x = [a, b] x }"),
        ("Just list", "fun f = { [a, b] }"),
        ("Empty body", "fun f = { }"),
        
        // Test with extra spaces/formatting
        ("Extra spaces", "fun   f   =   {   val   x   =   []   (x)   match   {   [a,   b]   =>   {   0   }   }   }"),
        
        // Test other patterns with similar length
        ("Different content", "fun f = { val x = 123456789012345678901234567890 x }"),
    ];
    
    for (name, input) in tests {
        println!("\n{} (length {}):", name, input.len());
        println!("Input: {:?}", input);
        
        // Try to parse just the top declaration
        match top_decl(input) {
            Ok((remaining, decl)) => {
                println!("✓ top_decl succeeded");
                println!("  Remaining: {} chars", remaining.len());
                println!("  Declaration type: {:?}", match decl {
                    restrict_lang::ast::TopDecl::Function(_) => "Function",
                    restrict_lang::ast::TopDecl::Record(_) => "Record",
                    restrict_lang::ast::TopDecl::Impl(_) => "Impl",
                    restrict_lang::ast::TopDecl::Context(_) => "Context",
                    restrict_lang::ast::TopDecl::Binding(_) => "Binding",
                });
            }
            Err(e) => {
                println!("✗ top_decl failed: {:?}", e);
            }
        }
        
        // Also try full parse_program
        match parse_program(input) {
            Ok((remaining, prog)) => {
                println!("✓ parse_program succeeded: {} declarations", prog.declarations.len());
            }
            Err(e) => {
                println!("✗ parse_program failed: {:?}", e);
            }
        }
    }
}