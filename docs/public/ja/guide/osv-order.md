# OSV語順

Restrict Languageの関数呼び出しは、v0.0.1ではOSV（目的語-主語-動詞）語順だけを使います。値や引数を先に書き、その後に関数名を書きます。

## OSVとは？

OSVは以下を表します：
- **O**bject（目的語）- 操作されるデータ
- **S**ubject（主語）- 関数または演算子
- **V**erb（動詞）- アクションまたは適用

Restrict Languageでは、単一引数はパイプ、複数引数はタプルで表します。

```restrict
value |> function
(first, second) combine
```

関数名を先に置く呼び出しや、オブジェクト風のメソッド呼び出しはv0.0.1の現在の構文ではありません。

## 基本的な使用法

### 単一引数関数

単一引数の関数には`|>`を使います：

```restrict
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun increment: (x: Int32) -> Int32 = {
    x + 1
}

fun main: () = {
    val result1 = 21 |> double
    val result2 = 41 |> increment

    result1 |> print_int
    result2 |> print_int
}
```

### 複数引数関数

複数引数の関数には、引数をタプルとして先に置きます：

```restrict
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun multiply: (x: Int32, y: Int32) -> Int32 = {
    x * y
}

fun main: () = {
    val sum = (5, 10) add
    val product = (3, 4) multiply
    val total = (sum, product) add
    total |> print_int
}
```

## 関数合成

OSVの連鎖では、データフローが左から右へ進みます：

```restrict
fun increment: (x: Int32) -> Int32 = {
    x + 1
}

fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun square: (x: Int32) -> Int32 = {
    x * x
}

fun main: () = {
    val result = 5
        |> increment
        |> double
        |> square

    result |> print_int
}
```

## パイプ演算子との併用

途中で複数引数の関数を使う場合は、必要な値をいったん束縛してタプル呼び出しに戻します：

```restrict
fun normalize: (value: Int32) -> Int32 = {
    value + 1
}

fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () = {
    val normalized = 10 |> normalize
    val total = (normalized, 5) add
    total |> print_int
}
```

## フィールドアクセスとプロトタイプ操作

フィールド参照は`.`を使えますが、関数呼び出しはOSVのままです。プロトタイプ更新には`clone`、不変化には`freeze`を使います。

```restrict
record Person {
    name: String
    age: Int32
}

fun greeting: (person: Person) -> String = {
    "Hello, " + person.name
}

fun main: () = {
    val alice = Person { name: "Alice", age: 30 }
    val message = alice |> greeting
    message |> println

    val bob = Person { name: "Bob", age: 24 }
    val older_bob = bob.clone { age: 25 }
    val frozen_bob = older_bob freeze
    frozen_bob
}
```

## v0.0.1では現在対象外の書き方

次のような書き方は、古い資料や将来構想では見かけることがありますが、v0.0.1の現在の公開ドキュメントではRestrictコードとして示しません。

- 関数名を先に書く呼び出し
- ドット記法でメソッドを直接呼ぶ書き方
- プレースホルダー引数を使った部分適用
- パイプで新しい変数名を作る束縛構文
- 可変パイプ構文
- `use`形式や文字列パスのインポート

## 良い使用例

### データ変換パイプライン

```restrict
fun validate: (value: Int32) -> Int32 = {
    value
}

fun normalize: (value: Int32) -> Int32 = {
    value + 1
}

fun transform: (value: Int32) -> Int32 = {
    value * 2
}

fun main: () = {
    val raw_data = 10
    val result = raw_data
        |> validate
        |> normalize
        |> transform

    result |> print_int
}
```

### 数学的操作

```restrict
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun increment: (x: Int32) -> Int32 = {
    x + 1
}

fun square: (x: Int32) -> Int32 = {
    x * x
}

fun main: () = {
    val result = 10
        |> double
        |> increment
        |> square

    result |> print_int
}
```

### 条件式との組み合わせ

```restrict
fun choose_label: (score: Int32) -> String = {
    score >= 80 then { "pass" } else { "review" }
}

fun main: () = {
    val label = 92 |> choose_label
    label |> println
}
```

## 実装の詳細

### パース優先順位

フィールドアクセス、算術演算、比較演算のあとにパイプが適用されます。曖昧になりそうな式では括弧を使います。

```restrict
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () = {
    val first = (5 + 1) |> double
    val second = (5, 3 |> double) add
    val total = (first, second) add
    total |> print_int
}
```

### 型推論

OSVは型推論と組み合わせて使えます。明示したい境界では型注釈を付けます。

```restrict
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun main: () = {
    val input: Int32 = 21
    val output = input |> double
    output |> print_int
}
```

## ベストプラクティス

1. **OSVに統一する** - v0.0.1のRestrictコードでは関数呼び出しをOSVだけで書く
2. **単一引数はパイプを使う** - `value |> function`でデータフローを明確にする
3. **複数引数はタプルを使う** - `(a, b) function`で引数を先に置く
4. **複雑な式は分割する** - 中間値に名前を付けて読みやすくする
5. **将来構想の構文を混ぜない** - 現在の公開サーフェスだけをコード例に使う

## 関連項目

- [関数](functions.md) - 関数定義と呼び出し
- [パイプ演算子](../reference/operators.md#pipe) - `|>`演算子
- [関数合成](../advanced/composition.md) - 高度な合成パターン
