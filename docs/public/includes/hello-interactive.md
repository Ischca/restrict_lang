```restrict
import host.console.{read_line}

fun main: () -> () = {
    "あなたの名前は？ " |> print

    val name = () read_line
    val greeting = "こんにちは、" + name + "さん！"

    greeting |> println
}
```
