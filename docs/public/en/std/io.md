# I/O Operations

Console output uses the compiler's standard `Display` form:

```text
display: <T of Display>(T) -> String
print: <T of Display>(T) -> ()
println: <T of Display>(T) -> ()
eprint: (String) -> ()
eprintln: (String) -> ()
```

`String`, `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `Unit` have
built-in Display adoptions. Records opt in explicitly with
`RecordName takes Display`. `print_int` and `print_float` remain compatibility
helpers; stderr output remains String-only.
