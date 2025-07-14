# 構文リファレンス

このガイドでは、基本的な式から高度な機能まで、Restrict Languageの完全な構文を説明します。

## コメント

```restrict
// 単一行コメント

/* 
   複数行コメント
   複数行にまたがることができます
*/

/// 次の項目のドキュメントコメント
/// Markdownフォーマットをサポート
fn documented_function() { ... }
```

## 識別子とキーワード

### 識別子

識別子は文字またはアンダースコアで始まり、その後に文字、数字、またはアンダースコアが続きます：

```restrict
let valid_name = 1;
let _private = 2;
let camelCase = 3;
let snake_case = 4;
let number123 = 5;
```

### キーワード

以下は予約キーワードです：

```
let mut fn type struct enum match if else while for 
loop break continue return clone freeze derive from
with as impl trait pub mod use import export true false
```

## リテラル

### 数値

```restrict
// 整数
let decimal = 42;
let hex = 0xFF;
let octal = 0o77;
let binary = 0b1010;
let with_underscores = 1_000_000;

// 浮動小数点
let float = 3.14;
let scientific = 2.5e-10;
```

### 文字列

```restrict
// 文字列リテラル
let simple = "Hello, World!";
let escaped = "Line 1\nLine 2\tTabbed";
let unicode = "Unicode: \u{1F44B}";

// 生文字列
let raw = r"エスケープなし\n";
let raw_hashes = r#""引用符"を含むことができます"#;

// 複数行文字列
let multiline = """
    これは
    複数行文字列で
    フォーマットが保持されます
""";
```

### 文字

```restrict
let ch = 'a';
let unicode_ch = '🦀';
let escaped_ch = '\n';
```

### ブール値

```restrict
let yes = true;
let no = false;
```

## 変数と束縛

### 不変束縛

```restrict
let x = 42;          // 型推論
let y: i32 = 42;     // 明示的な型
let (a, b) = (1, 2); // パターン分解
```

### 可変束縛

```restrict
let mut counter = 0;
counter = counter + 1;  // 変更可能

// 可変パイプ演算子
let mut data = getData();
data |>> process;  // インプレース変更
```

## 式

### 算術演算

```restrict
let sum = 1 + 2;
let difference = 5 - 3;
let product = 4 * 3;
let quotient = 10 / 2;
let remainder = 7 % 3;
let power = 2 ** 8;
```

### 比較演算

```restrict
let equal = x == y;
let not_equal = x != y;
let less = x < y;
let greater = x > y;
let less_eq = x <= y;
let greater_eq = x >= y;
```

### 論理演算

```restrict
let and_result = true && false;
let or_result = true || false;
let not_result = !true;
```

### ビット演算

```restrict
let bit_and = 0b1100 & 0b1010;  // 0b1000
let bit_or = 0b1100 | 0b1010;   // 0b1110
let bit_xor = 0b1100 ^ 0b1010;  // 0b0110
let bit_not = ~0b1010;           // ビット否定
let shift_left = 1 << 3;         // 8
let shift_right = 8 >> 2;        // 2
```

## 制御フロー

### if式

```restrict
// 基本的なif
if condition {
    doSomething();
}

// if-else
let result = if x > 0 {
    "正"
} else if x < 0 {
    "負"
} else {
    "ゼロ"
};

// 条件でのパターンマッチング
if let Some(value) = optional {
    value |> process;
}
```

### match式

```restrict
// 基本的なmatch
let description = match number {
    0 => "ゼロ",
    1 => "一",
    2..=5 => "二から五",
    _ => "その他"
};

// ガード付きパターンマッチング
match value {
    Some(x) if x > 0 => x |> process,
    Some(x) => x |> handleNegative,
    None => defaultValue()
}

// パターンでの分解
match point {
    { x: 0, y: 0 } => "原点",
    { x: 0, y } => "y軸上の" ++ y.toString(),
    { x, y: 0 } => "x軸上の" ++ x.toString(),
    { x, y } => "(" ++ x.toString() ++ ", " ++ y.toString() ++ ")の位置"
}
```

### ループ

```restrict
// whileループ
while condition {
    doWork();
}

// 範囲でのforループ
for i in 0..10 {
    i |> println;
}

// コレクションでのforループ
for item in list {
    item |> process;
}

// breakを使ったループ
loop {
    if done() {
        break;
    }
    continue;
}

// ループラベル
'outer: loop {
    'inner: loop {
        if condition {
            break 'outer;
        }
    }
}
```

## 関数

### 関数定義

```restrict
// 基本的な関数
fn add(x: i32, y: i32) -> i32 {
    x + y
}

// ジェネリック関数
fn identity<T>(value: T) -> T {
    value
}

// where句を持つ関数
fn process<T>(data: T) -> String 
    where T: ToString
{
    data.toString()
}

// OSVスタイルの関数呼び出し
42 |> add(10);  // add(42, 10)
"hello" |> process;
```

### ラムダ式

```restrict
// シンプルなラムダ
let add_one = |x| x + 1;

// 型注釈付き
let multiply: fn(i32, i32) -> i32 = |x, y| x * y;

// 変数のキャプチャ
let factor = 10;
let scale = |x| x * factor;

// 高階関数での使用
list |> map(|x| x * 2) |> filter(|x| x > 10);
```

## 型

### プリミティブ型

```restrict
// 整数
i8, i16, i32, i64, i128
u8, u16, u32, u64, u128

// 浮動小数点
f32, f64

// ブール値
bool

// 文字
char

// 文字列（アフィン型）
String
```

### 複合型

```restrict
// 配列（固定サイズ）
let array: [i32; 5] = [1, 2, 3, 4, 5];

// スライス（配列のビュー）
let slice: &[i32] = &array[1..4];

// タプル
let tuple: (i32, String, bool) = (42, "hello", true);
let (x, y, z) = tuple;  // 分解

// Option型
let some_value: Option<i32> = Some(42);
let no_value: Option<i32> = None;

// Result型
let success: Result<i32, String> = Ok(42);
let failure: Result<i32, String> = Err("エラーメッセージ");
```

### カスタム型

```restrict
// 構造体
struct Point {
    x: f64,
    y: f64
}

// タプル構造体
struct Color(u8, u8, u8);

// 列挙型
enum Status {
    Active,
    Inactive,
    Pending { since: DateTime }
}

// 型エイリアス
type Distance = f64;
type Callback = fn(Event) -> bool;
```

## パターンマッチング

### パターン

```restrict
// リテラルパターン
match x {
    0 => "ゼロ",
    1 => "一",
    _ => "その他"
}

// 変数パターン
let Some(value) = optional;

// ワイルドカードパターン
let (first, _, third) = triple;

// 範囲パターン
match score {
    0..=59 => "F",
    60..=69 => "D",
    70..=79 => "C",
    80..=89 => "B",
    90..=100 => "A",
    _ => "無効"
}

// 構造体パターン
let Point { x, y } = point;
let Point { x: px, y: py } = point;  // リネーム

// ガード句
match value {
    Some(x) if x > 0 => "正",
    Some(x) if x < 0 => "負",
    Some(_) => "ゼロ",
    None => "なし"
}
```

## モジュールとインポート

```restrict
// モジュール定義
mod math {
    pub fn add(x: i32, y: i32) -> i32 {
        x + y
    }
    
    pub mod advanced {
        pub fn pow(base: f64, exp: f64) -> f64 {
            base ** exp
        }
    }
}

// インポート
use std::collections::List;
use math::add;
use math::advanced::pow;

// エイリアス付きインポート
use std::string::String as Str;

// グロブインポート
use std::prelude::*;
```

## 属性

```restrict
// 関数属性
#[inline]
fn fast_function() { ... }

#[test]
fn test_addition() {
    assert_eq!(2 + 2, 4);
}

// Derive属性
#[derive(Debug, Clone)]
struct Point { x: f64, y: f64 }

// モジュール属性
#[cfg(test)]
mod tests {
    // テストモジュール
}
```

## 特殊構文

### withブロック（リソース管理）

```restrict
with file = openFile("data.txt") {
    file |> readContents |> process;
}  // ファイルは自動的に閉じられる

with db = connectDatabase(url) {
    db |> query("SELECT * FROM users");
}  // 接続は自動的に閉じられる
```

### クローンとフリーズ

```restrict
// クローンは可変コピーを作成
let original = { x: 10, y: 20 };
let mut copy = clone original;
copy.x = 30;  // OK

// フリーズは不変プロトタイプを作成
let prototype = freeze { x: 10, y: 20 };
let instance = clone prototype;
// prototypeは変更できない
```

### 派生境界

```restrict
// 派生境界を持つジェネリック
fn process<T from Base>(value: T) -> Result<String> {
    // TはBaseプロトタイプから派生している必要がある
    value |> validate |> transform
}
```

## 演算子の優先順位

1. メンバーアクセス: `.`
2. 関数呼び出し、配列インデックス
3. 単項演算子: `-`, `!`, `~`
4. べき乗: `**`
5. 乗除余: `*`, `/`, `%`
6. 加減: `+`, `-`
7. シフト: `<<`, `>>`
8. ビットAND: `&`
9. ビットXOR: `^`
10. ビットOR: `|`
11. 比較: `<`, `>`, `<=`, `>=`
12. 等価: `==`, `!=`
13. 論理AND: `&&`
14. 論理OR: `||`
15. 範囲: `..`, `..=`
16. 代入: `=`
17. パイプ: `|>`, `|>>`

## まとめ

この構文リファレンスは、Restrict Languageの基本要素をカバーしています。構文は以下のように設計されています：

- Rustプログラマーにとって**親しみやすい**
- OSV語順で**自然**
- アフィン型で**安全**
- 関数型プログラミングに**表現力豊か**

より詳細な例とパターンについては、[言語ガイド](./README.md)を参照してください。