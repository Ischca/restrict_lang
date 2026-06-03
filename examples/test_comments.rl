// Simple test of comment support

/* Multi-line
   comment test */

/* Small function
   with inferred return type */
fun double: (n: Int32) = {
    n * 2
}

fun main: () -> Int32 = {
    // Create a simple value
    val x = 42;  // This is an integer

    // Apply the function
    val result = x |> double;  /* Should be 84 */

    result  // Return the result
}
