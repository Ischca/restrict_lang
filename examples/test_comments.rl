// Simple test of comment support

/* Multi-line
   comment test */

fun main = {
    // Create a simple value
    val x = 42;  // This is an integer
    
    /* Create a lambda
       with type inference */
    val double = |n| n * 2;  // n inferred as Int32
    
    // Apply the lambda
    val result = (x) double;  /* Should be 84 */
    
    result  // Return the result
}