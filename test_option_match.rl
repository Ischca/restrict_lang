fun main = {
    val x = 42 some;
    val y = None<Int>;
    
    // Test pattern matching on Some
    match x {
        Some(n) => n println;
        None => "Got None" println;
    };
    
    // Test pattern matching on None
    match y {
        Some(n) => n println;
        None => "Got None for y" println;
    };
    
    // Test with inline values
    match 100 some {
        Some(n) => n println;
        None => "Never happens" println;
    };
    
    0
}