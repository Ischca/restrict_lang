```restrict
use std::io::readLine;

fn main() {
    "あなたの名前は？ " |> print
    
    let name = readLine()
    
    "こんにちは、" ++ name ++ "さん！" |> println
}
```