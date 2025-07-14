```restrict
fn main() {
    // 名前を定義
    let name = "Restrict"
    
    // 文字列を連結して出力
    "Hello, " ++ name ++ "!" |> println
    
    // 複数の操作をチェーン
    name
        |> toUpperCase
        |> println
}
```