# 変数と可変性

Restrict Languageでは、変数は**アフィン型システム**に従います。これは、各変数が最大1回しか使用できないことを意味します。この設計により、多くの一般的なプログラミングエラーが排除され、ガベージコレクションなしで予測可能なメモリ管理が実現されます。

## 不変変数

デフォルトでは、Restrict Languageのすべての変数は不変でアフィンです：

```restrict
val x = 42
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
val mut counter = 0
counter = counter + 1  // OK: 可変変数は再代入できます
counter = counter + 1  // OK: 複数回使用できます
val final_count = counter  // counterはここで消費されます
```

## `|>`による変数束縛

パイプ演算子`|>`は不変の束縛を作成します：

```restrict
42 |> x       // 42をxに束縛
|> double     // xをdouble関数に渡す
|> result     // 結果を束縛

// 以下と同等：
val x = 42
val temp = x double
val result = temp
```

## `|>>`による可変束縛

可変束縛には、ダブルパイプを使用します：

```restrict
0 |>> mut counter
counter = counter + 1
counter = counter + 1
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
val (a, b) = (10, 20)  // タプルの分解
val Person { name, age } = get_person()  // レコードの分解

// match式内で
some_option match {
    Some(value) => { value * 2 }  // valueはここで束縛される
    None => { 0 }
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
fun sum_list = lst:List<Int>, acc:Int {
    lst match {
        [] => { acc }
        [head | tail] => { tail (acc + head) sum_list }
    }
}
```

### ビルダーパターン
```restrict
Person { name: "Alice", age: 0 }
|> set_age(25)
|> set_email("alice@example.com")
|> build
```

## 関連項目

- [アフィン型](affine-types.md) - アフィン型システムの詳細
- [関数](functions.md) - 関数と変数の相互作用
- [パターンマッチング](../advanced/patterns.md) - 高度なパターン束縛