// OSV syntax examples
fn processData(data: Vec<String>) -> Result<String, Error> {
    data
        |> filter(|s| !s.isEmpty())
        |> map(|s| s |> toUpperCase)
        |> join(", ")
        |> Ok
}

// Pipe operator chains
fn pipeExample() {
    "hello"
        |> toUpperCase      // "HELLO"
        |> reverse          // "OLLEH"
        |> println          // Prints: OLLEH
}