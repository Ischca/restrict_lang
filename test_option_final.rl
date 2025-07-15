fun main = {
    val x = 42 some;
    val y = None<Int>;
    
    // Test pattern matching on Some
    x match {
        Some(n) => { n println }
        None => { "Got None" println }
    };
    
    // Test pattern matching on None
    y match {
        Some(n) => { n println }
        None => { "Got None for y" println }
    };
    
    // Test with inline values
    100 some match {
        Some(n) => { n println }
        None => { "Never happens" println }
    };
    
    0
}