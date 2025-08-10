// Test method resolution for records

record Point {
    x: Int32
    y: Int32
}

fun distance: (p: Point) -> Int32 = {
    // For now, just return x field due to affine restrictions
    p.x
}

fun main: () = {
    val p = Point { x: 3, y: 4 };
    
    // Test field access
    p.x |> println;
    p.y |> println;
    
    // Test method call (if it works)
    val d = p |> distance;
    d |> println;
}