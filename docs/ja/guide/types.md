# 型システム

Restrict Languageは、静的型付け、メモリ安全性のためのアフィン型、そして強力な型推論を組み合わせた洗練された型システムを特徴としています。このガイドでは、型システムを詳しく探求します。

## アフィン型

Restrict Languageの最も特徴的な機能は、アフィン型システムです。アフィン型は、値が**最大1回まで**使用できることを保証します。

### アフィン型とは？

```restrict
let message = "Hello";
message |> println;     // 所有権がprintlnに転送される
// message |> println;  // エラー: messageは既に消費されている
```

これにより、以下のような一般的なバグを防ぎます：
- Use-after-free
- Double-free
- データ競合

### 型が消費されるタイミング

値は以下の場合に消費されます：

1. **関数に渡されたとき**
```restrict
let data = getData();
data |> process;  // dataが消費される
// dataはもう利用できない
```

2. **別の変数に代入されたとき**
```restrict
let x = createResource();
let y = x;  // xが消費される
// xはもう利用できない
```

3. **関数から返されたとき**
```restrict
fn transfer(resource: Resource) -> Resource {
    resource  // 所有権が呼び出し元に転送される
}
```

### アフィン型の操作

#### クローン

値を複数回使用する必要がある場合は、`clone`を使用します：

```restrict
let original = "Hello";
let copy = clone original;

original |> println;  // OK
copy |> println;      // OK
```

#### パターンマッチング

アフィン型でのパターンマッチング：

```restrict
let result = compute();
match result {
    Ok(value) => value |> process,  // このブランチでvalueが消費される
    Err(error) => error |> logError  // このブランチでerrorが消費される
}
// resultは完全に消費される
```

## プリミティブ型

### 数値型

```restrict
// 符号付き整数
let i8_val: i8 = -128;
let i16_val: i16 = -32768;
let i32_val: i32 = -2147483648;
let i64_val: i64 = -9223372036854775808;
let i128_val: i128 = -170141183460469231731687303715884105728;

// 符号なし整数
let u8_val: u8 = 255;
let u16_val: u16 = 65535;
let u32_val: u32 = 4294967295;
let u64_val: u64 = 18446744073709551615;
let u128_val: u128 = 340282366920938463463374607431768211455;

// 浮動小数点
let f32_val: f32 = 3.14159;
let f64_val: f64 = 2.718281828459045;

// プラットフォーム依存
let size: usize = 100;  // ポインタサイズの符号なし
let diff: isize = -50;  // ポインタサイズの符号付き
```

### ブール型

```restrict
let is_ready: bool = true;
let is_finished: bool = false;

// ブール演算
let both = is_ready && is_finished;
let either = is_ready || is_finished;
let not_ready = !is_ready;
```

### 文字型

```restrict
let letter: char = 'A';
let emoji: char = '😀';
let unicode: char = '\u{1F600}';
```

### ユニット型

ユニット型 `()` は空の値を表します：

```restrict
fn do_nothing() -> () {
    // ユニットを返す
}

let unit_value: () = ();
```

## 文字列型

### String（所有）

`String`は所有されたUTF-8テキストを表すアフィン型です：

```restrict
let mut greeting: String = "Hello";
greeting = greeting ++ ", World!";  // 連結

// Stringは使用時に消費される
greeting |> println;
// greetingはもう利用できない
```

### &str（文字列スライス）

文字列スライスは文字列への借用ビューです：

```restrict
let full_name = "John Doe";
let first_name: &str = &full_name[0..4];  // "John"
```

## 複合型

### 配列

固定サイズの要素のシーケンス：

```restrict
let numbers: [i32; 5] = [1, 2, 3, 4, 5];
let zeros: [i32; 100] = [0; 100];  // 100個のゼロ

// 配列アクセス
let first = numbers[0];
let last = numbers[4];
```

### スライス

配列への動的ビュー：

```restrict
let array = [1, 2, 3, 4, 5];
let slice: &[i32] = &array[1..4];  // [2, 3, 4]

// スライス操作
slice |> len;      // 3
slice[0];          // 2
```

### タプル

固定サイズの異種コレクション：

```restrict
let person: (String, i32, bool) = ("Alice", 30, true);
let (name, age, active) = person;  // 分解

// タプル要素へのアクセス
let coordinates: (f64, f64) = (10.5, 20.7);
let x = coordinates.0;
let y = coordinates.1;
```

### ベクタ

動的配列（アフィン型）：

```restrict
let mut vec: Vec<i32> = Vec::new();
vec |>> push(1);
vec |>> push(2);
vec |>> push(3);

// ベクタはイテレート時に消費される
vec |> iter |> map(|x| x * 2) |> collect;
```

## カスタム型

### 構造体

名前付きフィールドのコレクション：

```restrict
struct User {
    name: String,
    email: String,
    age: u32,
    active: bool
}

// インスタンスの作成
let user = User {
    name: "Alice",
    email: "alice@example.com",
    age: 30,
    active: true
};

// フィールドアクセス
let name = clone user.name;  // userを消費しないようにクローン
```

### タプル構造体

名前なしフィールドを持つ構造体：

```restrict
struct Point(f64, f64);
struct Color(u8, u8, u8);

let origin = Point(0.0, 0.0);
let red = Color(255, 0, 0);

// フィールドへのアクセス
let x = origin.0;
let r = red.0;
```

### 列挙型

バリアントを持つ直和型：

```restrict
enum Result<T, E> {
    Ok(T),
    Err(E)
}

enum Message {
    Quit,
    Move { x: i32, y: i32 },
    Write(String),
    ChangeColor(u8, u8, u8)
}

// パターンマッチング
let msg = Message::Move { x: 10, y: 20 };
match msg {
    Message::Quit => quit(),
    Message::Move { x, y } => moveTo(x, y),
    Message::Write(text) => text |> display,
    Message::ChangeColor(r, g, b) => setColor(r, g, b)
}
```

## 型エイリアス

型の代替名を作成：

```restrict
type UserId = u64;
type Result<T> = Result<T, String>;
type Callback = fn(Event) -> bool;

let id: UserId = 12345;
let result: Result<i32> = Ok(42);
```

## OptionとResult

### Option型

オプショナルな値を表現：

```restrict
enum Option<T> {
    Some(T),
    None
}

// Optionの使用
let maybe_number: Option<i32> = Some(42);
let nothing: Option<i32> = None;

// パターンマッチング
match maybe_number {
    Some(n) => n |> process,
    None => handleMissing()
}

// Optionメソッド
maybe_number |> map(|n| n * 2);
maybe_number |> unwrap_or(0);
```

### Result型

成功または失敗を表現：

```restrict
enum Result<T, E> {
    Ok(T),
    Err(E)
}

// Resultの使用
let result: Result<i32, String> = Ok(42);
let error: Result<i32, String> = Err("失敗");

// エラーハンドリング
result
    |> map(|n| n * 2)
    |> map_err(|e| "エラー: " ++ e)
    |> unwrap_or_else(|_| 0);
```

## ジェネリック型

### ジェネリック関数

```restrict
fn identity<T>(value: T) -> T {
    value
}

fn swap<A, B>(pair: (A, B)) -> (B, A) {
    let (a, b) = pair;
    (b, a)
}
```

### ジェネリック構造体

```restrict
struct Container<T> {
    value: T
}

impl<T> Container<T> {
    fn new(value: T) -> Container<T> {
        Container { value }
    }
    
    fn get(self) -> T {
        self.value  // コンテナを消費
    }
}
```

### 型制約

```restrict
fn display<T: ToString>(value: T) {
    value |> toString |> println;
}

fn process<T>(items: Vec<T>) -> Vec<String>
    where T: ToString + Clone
{
    items |> map(|item| item |> toString) |> collect
}
```

## 型推論

Restrict Languageは強力な型推論を持っています：

```restrict
// コンパイラが型を推論
let x = 42;           // i32
let y = 3.14;         // f64
let z = "hello";      // &str
let vec = vec![1, 2, 3];  // Vec<i32>

// 部分的な型注釈
let numbers: Vec<_> = vec![1, 2, 3];
let result = parse::<i32>("42");
```

## プロトタイプベースの型

Restrict Languageはプロトタイプベースの継承をサポート：

```restrict
// プロトタイプを作成
let animal_proto = freeze {
    species: "不明",
    makeSound: fn() { "..." |> println }
};

// プロトタイプから派生
let dog = clone animal_proto with {
    species: "犬",
    makeSound: fn() { "ワン！" |> println }
};

// 派生境界を持つ型
fn feed<T from animal_proto>(animal: T) {
    animal.species |> println;
    animal.makeSound();
}
```

## メモリ安全性

アフィン型システムは、ガベージコレクションなしでメモリ安全性を保証します：

```restrict
// リソース管理
with file = openFile("data.txt") {
    file |> read |> process;
}  // ファイルは自動的に閉じられる

// ダブルフリーなし
let resource = allocate();
resource |> use;
// resource |> use;  // エラー: 既に消費されている

// Use-after-freeなし
let data = getData();
let processed = data |> transform;  // dataが消費される
// data |> print;  // エラー: dataはもう利用できない
```

## ベストプラクティス

1. **アフィン型を受け入れる** - コンパイル時にバグを防ぐ
2. **cloneは控えめに使う** - 本当に複数回使用が必要な場合のみ
3. **型推論を活用する** - ただし明確性のために注釈を追加
4. **網羅的にパターンマッチする** - コンパイラがすべてのケースを保証
5. **OptionとResultを使う** - 明示的なエラーハンドリングのため

## 高度なトピック

### ファントム型

```restrict
struct Distance<Unit> {
    value: f64,
    _unit: PhantomData<Unit>
}

struct Meters;
struct Feet;

let d1: Distance<Meters> = Distance::new(100.0);
let d2: Distance<Feet> = Distance::new(328.0);
// 単位を誤って混ぜることはできない
```

### 関連型

```restrict
trait Container {
    type Item;
    fn get(self) -> Self::Item;
}

impl Container for Box<T> {
    type Item = T;
    fn get(self) -> T {
        self.value
    }
}
```

## まとめ

Restrict Languageの型システムは以下を提供します：
- アフィン型による**メモリ安全性**
- ジェネリクスと型推論による**表現力**
- ゼロコスト抽象化による**パフォーマンス**
- 網羅的パターンマッチングによる**正確性**

アフィン型とOSV構文の組み合わせは、コンパイル時にバグを捕捉しながら、人間工学的で表現力豊かなプログラミング体験を生み出します。