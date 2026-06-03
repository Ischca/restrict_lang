```restrict
import host.console.{read_line}
import host.string.{parse_int}

// 演習 1: 数値の合計
fun exercise1: () -> () = {
    "最初の数値を入力: " |> print
    val num1 = () read_line |> parse_int

    "二番目の数値を入力: " |> print
    val num2 = () read_line |> parse_int

    val sum = num1 + num2
    "合計: " |> print
    sum |> print_int
}

// 演習 2: リストの処理
fun exercise2: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3, 4, 5]
    val doubled = (numbers, |n| n * 2) map
    val filtered = (doubled, |n| n > 5) filter
    (filtered, 0, |total, n| total + n) fold
}

// 演習 3: エラー処理
fun safe_divide: (a: Int32, b: Int32) -> Result<Int32, String> = {
    b == 0 then {
        Err("division_by_zero")
    } else {
        Ok(a / b)
    }
}

fun exercise3: () -> () = {
    val result = (10, 0) safe_divide

    result match {
        Ok(value) => {
            "結果: " |> print
            value |> print_int
        }
        Err(message) => {
            "エラー: " |> print
            message |> println
        }
    }
}
```
