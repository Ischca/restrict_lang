```restrict
// OSV syntax demonstration
fn processData(data: Vec<String>) -> Result<String, Error> {
    data
        |> filter(|s| !s.isEmpty())
        |> map(|s| s |> toUpperCase)
        |> join(", ")
        |> Ok
}
```