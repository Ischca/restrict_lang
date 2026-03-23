```restrict
// 従来: function(data)
// Restrict: data function (OSV: Object Subject Verb)

fun double: (n: Int) -> Int = {
    n * 2
}

fun main = {
    5 double int_to_string |> println
    10 |> double |> int_to_string |> println
}
```
