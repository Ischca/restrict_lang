```restrict
// OSV syntax: data flows left-to-right

fun double: (n: Int) -> Int = {
    n * 2
}

fun add_one: (n: Int) -> Int = {
    n + 1
}

fun main = {
    // Chain with pipes: 5 -> 10 -> 11
    5 |> double |> add_one |> int_to_string |> println
}
```
