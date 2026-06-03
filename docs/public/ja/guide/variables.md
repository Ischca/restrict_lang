# 変数と可変性

Restrict Languageでは、変数は**アフィン型システム**に従います。これは、各変数が最大1回しか使用できないことを意味します。この設計により、多くの一般的なプログラミングエラーが排除され、ガベージコレクションなしで予測可能なメモリ管理が実現されます。

## 不変変数

デフォルトでは、Restrict Languageのすべての変数は不変でアフィンです：

```restrict
val x = "owned value"
val y = x    // xはyに移動され、もうアクセスできません
// val z = x // エラー: xはすでに使用されています！
```

この単一使用ルールはすべての型に適用されます：

```restrict
val message = "Hello"
message |> println    // messageはここで消費されます
// message |> println // エラー: messageはすでに使用されています！
```

## 可変変数

値を変更する必要がある場合は、`mut`を使用します：

```restrict
mut val counter = 0
counter = counter + 1  // OK: 可変変数は再代入できます
counter = counter + 1  // OK: 複数回使用できます
val final_count = counter  // counterはここで消費されます
```

## `|>`による関数適用

パイプ演算子`|>`は、値を単一引数の関数へ渡します。v0.0.1では、パイプで新しい変数名を作る束縛構文は現在の対象ではありません。

```restrict
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun main: () = {
    val x = 42
    val result = x |> double
    result |> print_int
}
```

## 可変束縛

可変束縛も通常の宣言として書きます。パイプ演算子の可変版はv0.0.1の構文ではありません。

```restrict
mut val counter = 0
counter = counter + 1
counter |> print_int
```

## シャドーイング

同じ名前で新しい束縛を作成することで、変数をシャドーイングできます：

```restrict
val x = 5
val x = x + 1  // 新しいxが古いxをシャドーイング
val x = "今は文字列です"  // シャドーイングで型も変更可能
```

## パターン束縛

パターンマッチングを通じて変数を束縛できます：

```restrict
fun process_option: (some_option: Option<Int32>) -> Int32 = {
    some_option match {
        Some(value) => { value * 2 }  // valueはここで束縛される
        None => { 0 }
    }
}
```

## ベストプラクティス

1. **デフォルトで不変変数を使用** - 必要な場合のみ`mut`を使用
2. **アフィンシステムに従う** - 値を複数回使用する必要がある場合は、関数パラメータとして渡すことを検討
3. **パイプ演算子を活用** - データフローを明示的で明確にする
4. **意味のある名前を使用** - 変数は一度しか使用されないことが多いため、説明的な名前を付ける

## 一般的なパターン

### アキュムレータパターン
```restrict
fun sum_list: (lst: List<Int32>, acc: Int32) -> Int32 = {
    lst match {
        [] => { acc }
        [head | tail] => { (tail, acc + head) sum_list }
    }
}
```

### ビルダーパターン
```restrict
record Person {
    name: String
    age: Int32
    email: String
}

fun main: () = {
    val base = Person { name: "Alice", age: 0, email: "" }
    val adult = base.clone { age: 25 }
    val ready = adult.clone { email: "alice@example.com" } freeze
    ready
}
```

## 関連項目

- [アフィン型](affine-types.md) - アフィン型システムの詳細
- [関数](functions.md) - 関数と変数の相互作用
- [パターンマッチング](../advanced/patterns.md) - 高度なパターン束縛
