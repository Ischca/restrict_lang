// This is a single-line comment
record Point { x: Int, y: Int }

/* This is a 
   multi-line comment */
fun main = {
    // Create a point
    val p = Point { x = 10, y = 20 } /* inline comment */
    
    // Calculate sum
    p.x + p.y  // another comment
}