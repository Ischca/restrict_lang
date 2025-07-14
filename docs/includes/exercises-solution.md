```restrict
use std::io::readLine;
use std::string::parse;

// 演習 1: 数値の合計
fn exercise1() {
    "最初の数値を入力: " |> print
    let num1 = readLine() |> parse::<i32> |> unwrap()
    
    "二番目の数値を入力: " |> print
    let num2 = readLine() |> parse::<i32> |> unwrap()
    
    let sum = num1 + num2
    "合計: " ++ sum.toString() |> println
}

// 演習 2: リストの処理
fn exercise2() {
    [1, 2, 3, 4, 5]
        |> map(|n| n * 2)
        |> filter(|n| n > 5)
        |> forEach(|n| n.toString() |> println)
}

// 演習 3: カスタムエラー処理
enum MathError {
    DivisionByZero,
    Overflow
}

fn safeDivide(a: i32, b: i32) -> Result<i32, MathError> {
    if b == 0 {
        Err(MathError::DivisionByZero)
    } else {
        Ok(a / b)
    }
}

fn exercise3() {
    match safeDivide(10, 0) {
        Ok(result) => "結果: " ++ result.toString() |> println,
        Err(MathError::DivisionByZero) => "エラー: ゼロ除算" |> println,
        Err(MathError::Overflow) => "エラー: オーバーフロー" |> println
    }
}
```