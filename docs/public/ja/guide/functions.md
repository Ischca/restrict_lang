# 関数

Restrict Language では、関数は OSV 構文で呼び出します。引数は関数名の前に置き、関数宣言は `fun name: (...) -> Type = { ... }` の形で書きます。

## 関数定義

基本的な関数構文:

```restrict
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}
```

パラメータなしの関数:

```restrict
fun get_answer: () -> Int32 = {
    42
}
```

戻り値型を推論させる関数:

```restrict
fun double: (x: Int32) = {
    x * 2
}
```

## OSV 呼び出し

```restrict
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun double: (x: Int32) -> Int32 = {
    x * 2
}

val total = (5, 10) add
val doubled = 21 |> double
```

単一引数では `|>` を使い、複数引数ではタプルを関数名の前に置きます。

## 関数合成

OSV では、値の流れを左から右へつなげます。

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

val result = 5 |> increment |> double |> square
```

## 高階関数

関数は他の関数を受け取れます。

```restrict
fun apply_twice: (value: Int32, f: Int32 -> Int32) -> Int32 = {
    value |> f |> f
}

fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun main: () -> Int32 = {
    (5, double) apply_twice
}
```

## ジェネリック関数

```restrict
fun identity: <T>(value: T) -> T = {
    value
}

fun map_option: <T, U>(opt: Option<T>, f: T -> U) -> Option<U> = {
    opt match {
        Some(value) => { Some(value |> f) }
        None => { None }
    }
}
```

## 関数値

ラムダ式を束縛して、通常の関数と同じ OSV 形で使えます。

```restrict
val add_five: Int32 -> Int32 = |x: Int32| x + 5
val multiply: (Int32, Int32) -> Int32 = |x: Int32, y: Int32| x * y

val result = 10 |> add_five
val product = (3, 4) multiply
```

## 分岐が返す関数値

`then`や`match`の各分岐がラムダで終わる場合、その値をいったん`val`に束縛し、後続のOSV呼び出しや`map`から関数型を推論できます。

```restrict
fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun main: (urgent: Boolean, bonus: Option<Int32>) -> Int32 = {
    val adjust = urgent then {
        val boost = 2
        |score| score + boost
    } else {
        val factor = 2
        |score| score * factor
    }
    val normalize = bonus match {
        Some(value) => {
            val doubled = value * 2
            |score| score + doubled
        }
        None => {
            val doubled = 0
            |score| score + doubled
        }
    }
    val scores = [10, 20]
    val adjusted = (scores, adjust) map
    val normalized = (adjusted, normalize) map

    (normalized, 0, add_int) fold
}
```

この分岐内の前置き処理は、v0.0.1では再実行しても安全な単純`val`束縛に限定します。`mut val`、複雑なパターン、`String`やレコード、リスト、関数値のような非Copy値はここでは拒否されます。

## 再帰関数

```restrict
fun factorial: (n: Int32) -> Int32 = {
    n <= 1 then {
        1
    } else {
        n * ((n - 1) |> factorial)
    }
}

fun fibonacci: (n: Int32) -> Int32 = {
    n match {
        0 => { 0 }
        1 => { 1 }
        _ => { ((n - 1) |> fibonacci) + ((n - 2) |> fibonacci) }
    }
}
```

## 部分適用に近い書き方

v0.0.1 では専用の部分適用構文を使わず、ラムダで必要な引数を固定します。

```restrict
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

val add_five: Int32 -> Int32 = |y: Int32| (5, y) add
val result = 10 |> add_five
```

## 関数型アノテーション

```restrict
val double_fn: Int32 -> Int32 = |x: Int32| x * 2
val add_fn: (Int32, Int32) -> Int32 = |x: Int32, y: Int32| x + y

fun transform: (value: Int32, f: Int32 -> Int32) -> Int32 = {
    value |> f
}
```

## v0.0.1 の current example ではないもの

次の機能は、仕様上予約済みまたは将来の設計対象です。このページでは current Restrict のコード例として扱いません。

- レコードに対するメソッド定義構文
- trait や impl による型クラス風の拡張
- 属性付きテスト関数
- 関数名を先に置く呼び出し形式
- 標準コレクション用の包括的な高階 API

## ベストプラクティス

1. パイプラインでは OSV を使い、値の流れを左から右へ保つ
2. 関数は小さく保ち、1つの変換や判定に集中させる
3. 複数引数はタプルを関数名の前に置く
4. 必要な型は `Int32`、`Float64`、`Boolean` など current type 名で書く
5. 迷った場合は `/LANGUAGE_SPECIFICATION.md` を確認する

## 関連項目

- [型推論](type-inference.md) - 関数型の推論方法
- [構文リファレンス](syntax.md) - v0.0.1 の基本構文
