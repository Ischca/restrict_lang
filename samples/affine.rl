// Affine Types — Use-at-most-once Semantics
//
// Every binding in Restrict can be used 0 or 1 times.
// This prevents aliasing bugs and enables safe memory management
// without a garbage collector.

fun greet: (name: String) -> String = {
    name
}

fun main: () -> () = {
    val message = "World"

    // First use — OK, this consumes 'message'
    message |> greet |> println

    // Uncommenting the next line would cause a compile error:
    // message |> greet |> println  // Error: 'message' already consumed
}
