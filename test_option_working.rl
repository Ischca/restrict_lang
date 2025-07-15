fun main = {
    val x = 42 some;
    val y = None<Int>;
    
    // Test pattern matching on Some
    x match {
        Some(n) => { n print_int }
        None => { "Got None" println }
    };
    
    // Test pattern matching on None
    y match {
        Some(n) => { n print_int }
        None => { "Got None for y" println }
    };
    
    // Test with inline values
    100 some match {
        Some(n) => { n print_int }
        None => { "Never happens" println }
    };
    
    0
}