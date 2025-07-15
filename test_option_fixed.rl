fun main = {
    val x = 42 some;
    val y = None<Int>;
    
    // Test pattern matching on Some - now returns values
    val result1 = x match {
        Some(n) => { n }
        None => { 0 }
    };
    result1 print_int;
    
    // Test pattern matching on None
    val result2 = y match {
        Some(n) => { n }
        None => { -1 }
    };
    result2 print_int;
    
    // Test with inline values
    val result3 = 100 some match {
        Some(n) => { n }
        None => { 0 }
    };
    result3 print_int;
    
    0
}