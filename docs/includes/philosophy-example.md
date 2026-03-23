```restrict
// 各値には単一の所有者がいます (Affine types)
fun main = {
    val x = 42
    val y = x     // x is consumed; ownership moves to y
    // x は使えません — y だけが有効です
    y int_to_string |> println
}
```
