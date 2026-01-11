# パターンマッチング

パターンマッチングは、複雑なデータ構造を分解し、マッチングを行うRestrict Languageの強力な機能です。この言語はmatch式でOSV（目的語-主語-動詞）構文に従います。

## 基本構文

Restrict Languageのmatch式はOSV構文を使用します：

```rust
expression match {
    pattern1 => { result1 }
    pattern2 => { result2 }
    _ => { default }
}
```

## リテラルパターン

リテラル値に対してマッチング：

```rust
val number = 42
number match {
    0 => { "ゼロ" }
    1 => { "一" }
    42 => { "答え" }
    _ => { "その他" }
}
```

## 変数束縛

マッチした値を変数に束縛：

```rust
val value = 10
value match {
    x => { x * 2 }  // 値をxに束縛
}
```

## Optionパターン

Option型に対するマッチング：

```rust
val maybe: Option<Int> = 42 some
maybe match {
    Some(value) => { value * 2 }
    None => { 0 }
}
```

## Resultパターン

Result型に対するマッチングでエラーハンドリング：

```rust
val result: Result<Int, String> = Ok(42)
result match {
    Ok(value) => { value * 2 }
    Err(msg) => { 0 }
}
```

Resultは失敗する可能性のある関数に便利です：

```rust
fun safe_divide = (a: Int, b: Int) -> Result<Int, String> {
    b == 0 then {
        Err("ゼロ除算")
    } else {
        Ok(a / b)
    }
}

val result = (10, 2) safe_divide
result match {
    Ok(n) => { n println }
    Err(msg) => { msg println }
}
```

## リストパターン

様々なパターンでリストを分解：

```rust
val numbers = [1, 2, 3, 4]

// 空リストパターン
[] match {
    [] => { "空" }
    _ => { "空ではない" }
}

// ヘッドとテールのパターン
numbers match {
    [] => { "空" }
    [head | tail] => { head }  // 1を返す
}

// 正確な長さのパターン
val pair = [1, 2]
pair match {
    [a, b] => { a + b }  // 3を返す
    _ => { 0 }
}
```

## レコードパターン

レコードを分解してフィールドを抽出：

```rust
record Point { x: Int y: Int }

val origin = Point { x: 0 y: 0 }
val point = Point { x: 10 y: 20 }

// 特定のフィールド値にマッチ
origin match {
    Point { x: 0 y: 0 } => { "原点" }
    Point { x y } => { x + y }
    _ => { "不明" }
}

// 短縮構文でのフィールド束縛
point match {
    Point { x y } => { x * y }  // xとyの両方が束縛される
}

// 明示的なフィールドパターン
point match {
    Point { x: px y: py } => { px + py }  // 異なる名前に束縛
}
```

## ネストしたパターン

複雑なマッチングのためのパターンの組み合わせ：

```rust
record Person { name: String age: Int }
record Company { name: String employees: List<Person> }

val company = Company {
    name: "Tech Corp"
    employees: [
        Person { name: "Alice" age: 30 }
        Person { name: "Bob" age: 25 }
    ]
}

company match {
    Company { name employees: [] } => { "従業員なし" }
    Company { name employees: [first | rest] } => { first.name }
    _ => { "不明" }
}
```

## 網羅性

型チェッカーはmatch式が網羅的であることを保証します。以下のいずれかが必要です：
- すべての可能なケースをカバーする
- ワイルドカードパターン（`_`）を含める

```rust
// これは型チェックに失敗します - 網羅的ではない
val opt: Option<Int> = None<Int>
opt match {
    Some(x) => { x }
    // Noneケースが欠落！
}

// これは正しい - 網羅的
opt match {
    Some(x) => { x }
    None => { 0 }
}
```

## 重要な注意事項

- すべてのパターン本体は中括弧 `{ }` で囲む必要があります
- match式はOSV構文に従います：`expression match { ... }`
- アフィン型システムにより、各束縛は最大1回しか使用できません
- パターンはコンパイル時に網羅性がチェックされます

## 将来の機能

- `then`条件を使用したパターンガード
- タプルパターン（タプルが実装された際）
- rest構文を使用した高度なリストパターン `[x, y, ...rest]`