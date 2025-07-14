```restrict
// Affine types prevent use-after-move
let message = "Hello"
message |> println     // OK: first use
// message |> println  // ERROR: already consumed

// Use clone when multiple uses needed
let original = "Hello"
let copy = clone original
original |> println    // OK
copy |> println       // OK
```