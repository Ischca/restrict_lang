```restrict
// Affine types: each value can be used at most once
fun main = {
    val x = 42
    x int_to_string |> println   // OK: first (and only) use

    // x |> println              // Error: x already consumed
}
```
