fun main = {
    val x = 42 some;
    val y = None<Int>;
    
    match x {
        Some(n) => n println;
        None => "Got None" println;
    };
    
    match y {
        Some(n) => n println;
        None => "Got None for y" println;
    };
    
    match 100 some {
        Some(n) => n println;
        None => "Never happens" println;
    };
    
    0
}