# OSV語順

Restrict Languageの特徴的な機能の1つは、OSV（目的語-主語-動詞）語順のサポートです。この構文により、自然で読みやすい関数合成とデータフローが可能になります。

## OSVとは？

OSVは以下を表します：
- **O**bject（目的語）- 操作されるデータ
- **S**ubject（主語）- 関数または演算子
- **V**erb（動詞）- アクションまたは適用

Restrict Languageでは、これは次のように変換されます：
```restrict
// 従来の構文: subject(object)
add(5, 10)

// OSV構文: object subject
(5, 10) add
```

## 基本的な使用法

### 単一引数関数

最も単純な形式では、OSVは単一引数関数で美しく機能します：

```restrict
fun double = x:Int { x * 2 }
fun increment = x:Int { x + 1 }

// 従来の呼び出し
val result1 = double(21)
val result2 = increment(41)

// OSV呼び出し
val result1 = 21 double      // 42
val result2 = 41 increment    // 42
```

### 複数引数関数

複数の引数の場合、タプル構文を使用します：

```restrict
fun add = x:Int, y:Int { x + y }
fun multiply = x:Int, y:Int { x * y }

// 従来の呼び出し
val sum = add(5, 10)
val product = multiply(3, 4)

// OSV呼び出し
val sum = (5, 10) add        // 15
val product = (3, 4) multiply // 12
```

## 関数合成

OSVが真に輝くのは、操作を連鎖させるときです：

```restrict
// 従来のネストした呼び出し
val result = square(double(increment(5)))

// OSVの連鎖
val result = 5 increment double square  // 144

// データフローが左から右へ明確に
// 5 → 6 → 12 → 144
```

## パイプ演算子との併用

OSVはパイプ演算子`|>`と自然に組み合わされます：

```restrict
val result = [1, 2, 3, 4, 5]
    |> filter(|x| x % 2 == 0)  // 偶数をフィルタ
    |> map(|x| x * x)          // 平方
    |> sum                     // 合計
    // 結果: 4 + 16 = 20
```

## メソッド呼び出し

メソッドは従来のドット記法とOSVの両方をサポートします：

```restrict
record Person {
    name: String,
    age: Int,
}

impl Person {
    fun greet = self:Person {
        "Hello, " + self.name
    }
    
    fun with_age = self:Person, years:Int {
        Person { name: self.name, age: years }
    }
}

val alice = Person { name: "Alice", age: 30 }

// メソッド呼び出し
val greeting1 = alice.greet()           // 従来の方法
val greeting2 = alice greet              // OSV

// 引数付きメソッド
val older1 = alice.with_age(35)         // 従来の方法
val older2 = (alice, 35) with_age       // OSV
```

## OSVを使用する場合

### 良い使用例

1. **データ変換パイプライン**
```restrict
raw_data
|> validate
|> normalize
|> transform
|> save
```

2. **数学的操作**
```restrict
val result = 10 double increment square  // ((10 * 2) + 1)²
```

3. **コレクション処理**
```restrict
numbers
|> filter(is_positive)
|> map(square)
|> take(10)
|> to_list
```

### 避けるべき場合

1. **複雑な引数構造**
```restrict
// 避ける: 読みにくい
((config, options), data) process_complex

// 推奨: 従来の構文を使用
process_complex(config, options, data)
```

2. **副作用のある操作**
```restrict
// 明確でない評価順序
x mutate y transform z combine  // 何が最初？

// より良い: 明示的な順序
val x_mut = x mutate
val y_trans = y transform
(x_mut, y_trans) combine
```

## 高度なパターン

### カリー化との組み合わせ

```restrict
fun add = x:Int { |y| x + y }

val add5 = 5 add         // 部分適用
val result = 10 add5     // 15
```

### ビルダーパターン

```restrict
Person::new()
|> set_name("Bob")
|> set_age(25)
|> set_email("bob@example.com")
|> build
```

### 条件付き連鎖

```restrict
val result = data
    |> validate
    |> (if needs_normalization 
        then normalize 
        else identity)
    |> process
```

## 実装の詳細

### パース優先順位

OSVはパース時に特別な優先順位を持ちます：
1. 括弧式が最初に評価される
2. OSV適用は左から右へ
3. 従来の関数呼び出しはOSVより優先順位が高い

```restrict
// パース順序の例
5 add double(3)     // 5 add 6 = 11
(5 add) double 3    // エラー: doubleは2引数を期待
5 add (3 double)    // 5 add 6 = 11
```

### 型推論

OSVは型推論とシームレスに動作します：

```restrict
// コンパイラは連鎖から型を推論
val result = "42" 
    |> parse_int    // String -> Option<Int>
    |> unwrap       // Option<Int> -> Int
    |> double       // Int -> Int
```

## ベストプラクティス

1. **一貫性を保つ** - プロジェクト全体でOSVまたは従来の構文を選択
2. **可読性を優先** - OSVがコードをより明確にする場合に使用
3. **パイプラインに使用** - データ変換の連鎖に最適
4. **シンプルに保つ** - 複雑な式には従来の構文を使用
5. **チームの慣習に従う** - チームのコーディング標準を確立

## 関連項目

- [関数](functions.md) - 関数定義と呼び出し
- [パイプ演算子](../reference/operators.md#pipe) - `|>`と`|>>`演算子
- [関数合成](../advanced/composition.md) - 高度な合成パターン