```restrict
// Affine types prevent use-after-move
val message = "Hello"
message |> println     // OK: first use
// message |> println  // ERROR: already consumed

// Record updates use postfix .clone syntax
record Greeting {
    text: String,
    count: Int32
}

val original = Greeting { text: "Hello", count: 1 }
val updated = original.clone { count: 2 }
updated.count |> print_int
```
