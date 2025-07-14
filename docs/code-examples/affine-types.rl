// Affine type examples
fn affineExample() {
    let message = "This can only be used once"
    
    // First use - OK
    message |> println
    
    // Second use - Compile error!
    // message |> println  // Error: message already consumed
}

// Using clone when needed
fn cloneExample() {
    let original = "Hello"
    let copy = clone original
    
    original |> println  // OK
    copy |> println      // OK
}