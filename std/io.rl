// Standard Library: IO
//
// Pure-Restrict formatting and console output helpers complementing
// compiler-registered IO builtins (print, println, print_int, print_float,
// eprint, eprintln, display).
//
// Console output helpers (`println_kv`, `eprintln_labeled`) target the
// `wasip1` profile; `format_kv` is pure and works in both `wasip1` and
// `wasm-core`.

pub fun format_kv: <T of Display>(key: String, value: T) -> String = {
    (key + ": ") + (value |> display)
}

pub fun println_kv: <T of Display>(key: String, value: T) -> () = {
    ((key, value) format_kv) |> println
}

pub fun eprintln_labeled: (label: String, message: String) -> () = {
    ((label + ": ") + message) |> eprintln
}
