# 関数

Restrict Languageでは、関数は第一級の値です。引数として渡したり、他の関数から返したり、変数に格納したりできます。

## 関数定義

基本的な関数構文：

```restrict
fun add = x:Int, y:Int -> Int {
    x + y
}
```

パラメータなしの関数：

```restrict
fun get_answer = -> Int {
    42
}
```

暗黙的な戻り値型の関数：

```restrict
fun double = x:Int {
    x * 2  // 型はIntと推論される
}
```

## OSV（目的語-主語-動詞）構文

Restrict LanguageはOSV構文をサポートし、自然な関数合成を可能にします：

```restrict
// 従来の呼び出し
val result = add(5, 10)

// OSV呼び出し - 目的語が先、次に主語（関数）
val result = (5, 10) add

// 単一引数のOSV
val doubled = 21 double
```

## 関数合成

OSVで関数を自然に連鎖させます：

```restrict
fun increment = x:Int { x + 1 }
fun double = x:Int { x * 2 }
fun square = x:Int { x * x }

// 操作の連鎖
val result = 5 increment double square  // ((5 + 1) * 2)² = 144
```

## 高階関数

関数は他の関数を受け取ったり返したりできます：

```restrict
fun apply_twice = f:(Int -> Int), x:Int {
    x f f  // OSVでfを2回適用
}

val quad = (double, 5) apply_twice  // 20を返す
```

## ジェネリック関数

複数の型で動作する関数を定義：

```restrict
fun identity<T> = x:T -> T {
    x
}

fun map_option<T, U> = opt:Option<T>, f:(T -> U) -> Option<U> {
    opt match {
        Some(value) => { Some(value f) }
        None => { None }
    }
}
```

## 関数値

関数は変数に格納できます：

```restrict
val add_five = |x| x + 5
val multiply = |x, y| x * y

// 通常の関数のように使用
val result = 10 add_five  // 15
val product = (3, 4) multiply  // 12
```

## 再帰関数

再帰関数は完全にサポートされています：

```restrict
fun factorial = n:Int -> Int {
    if n <= 1 
    then { 1 }
    else { n * (n - 1) factorial }
}

fun fibonacci = n:Int -> Int {
    n match {
        0 => { 0 }
        1 => { 1 }
        _ => { (n - 1) fibonacci + (n - 2) fibonacci }
    }
}
```

## メソッド構文

関数はレコードのメソッドとして定義できます：

```restrict
record Point {
    x: Int,
    y: Int,
}

impl Point {
    fun distance = self:Point, other:Point -> Int {
        val dx = self.x - other.x
        val dy = self.y - other.y
        // 簡略化された距離計算
        dx * dx + dy * dy
    }
}

// 使用方法
val p1 = Point { x: 0, y: 0 }
val p2 = Point { x: 3, y: 4 }
val dist = p1.distance(p2)  // メソッド呼び出し構文
```

## 部分適用

引数を部分的に適用して新しい関数を作成：

```restrict
fun add = x:Int, y:Int { x + y }

// ラムダを使った従来の部分適用
val add5 = |y| add(5, y)

// 使用
val result = 10 add5  // 15
```

## 関数型アノテーション

明示的な関数型構文：

```restrict
// Intを受け取りIntを返す関数
val double: (Int -> Int) = |x| x * 2

// 2つのIntを受け取りIntを返す関数
val add: (Int, Int -> Int) = |x, y| x + y

// 高階関数の型
val transformer: ((Int -> Int) -> (Int -> Int)) = |f| {
    |x| f(f(x))  // fを2回適用する関数を返す
}
```

## ベストプラクティス

1. **パイプラインにはOSVを使用** - 操作を連鎖させる場合、OSVはフローを明確にする
2. **関数を小さく保つ** - 各関数は1つのことをうまく行う
3. **純粋関数を優先** - 可能な限り副作用を避ける
4. **説明的な名前を使用** - 関数名は何をするかを説明すべき
5. **ジェネリック関数を検討** - ロジックが型に依存しない場合

## 一般的なパターン

### Filter-Map-Reduce
```restrict
[1, 2, 3, 4, 5]
|> filter(|x| x % 2 == 0)
|> map(|x| x * x)
|> fold(0, |acc, x| acc + x)
```

### 関数ビルダー
```restrict
fun make_multiplier = factor:Int {
    |x| x * factor  // クロージャを返す
}

val times_three = make_multiplier(3)
val result = 7 times_three  // 21
```

## 関連項目

- [ラムダ式](../advanced/lambdas.md) - 無名関数とクロージャ
- [高階関数](../advanced/higher-order.md) - 高度な関数パターン
- [型推論](type-inference.md) - 関数型の推論方法