```restrict
match value {
    Some(x) if x > 0 => x |> process,
    Some(x) => x |> handleNegative,
    None => defaultValue()
}
```