# 標準ライブラリリファレンス

Restrict Language標準ライブラリは、日常的なプログラミングタスクに必要な機能を提供します。すべての標準ライブラリモジュールは、アフィン型システムとOSV構文規則に従います。

## コアモジュール

### std::prelude

すべてのRestrictプログラムで自動的にインポートされる型と関数。

```restrict
// 自動的に利用可能:
println, print, clone, freeze, toString
Option, Result, Vec, String
```

### std::io

入出力操作。

```restrict
use std::io::*;

// 入力の読み取り
let input = readLine();
let content = readFile("data.txt")?;

// 出力の書き込み
"Hello" |> print;        // 改行なし
"World" |> println;      // 改行あり
content |> writeFile("output.txt")?;

// フォーマット出力
format!("名前: {}, 年齢: {}", name, age) |> println;
```

### std::string

文字列操作ユーティリティ。

```restrict
use std::string::*;

// 文字列操作
"hello" |> toUpperCase;      // "HELLO"
"WORLD" |> toLowerCase;      // "world"
"  trim me  " |> trim;       // "trim me"
"one,two,three" |> split(","); // ["one", "two", "three"]

// パース
"42" |> parse::<i32>();      // Ok(42)
"3.14" |> parse::<f64>();    // Ok(3.14)

// 文字列ビルダー
let mut sb = StringBuilder::new();
sb |>> append("Hello");
sb |>> append(" ");
sb |>> append("World");
sb |> build();  // "Hello World"
```

### std::collections

データ構造とコレクション。

```restrict
use std::collections::*;

// ベクター
let mut vec = Vec::new();
vec |>> push(1);
vec |>> push(2);
vec |> len();  // 2

// ハッシュマップ
let mut map = HashMap::new();
map |>> insert("key", "value");
map |> get("key");  // Some("value")

// ハッシュセット
let mut set = HashSet::new();
set |>> insert(1);
set |> contains(1);  // true

// リスト（関数型連結リスト）
let list = List::cons(1, List::cons(2, List::empty()));
list |> head();  // Some(1)
list |> tail();  // [2]を含むリスト
```

### std::iter

イテレータトレイトとユーティリティ。

```restrict
use std::iter::*;

// イテレータの作成
[1, 2, 3] |> iter();
1..10 |> iter();

// イテレータ操作
vec
    |> iter()
    |> map(|x| x * 2)
    |> filter(|x| x > 5)
    |> take(3)
    |> collect::<Vec<_>>();

// カスタムイテレータ
struct Counter {
    count: u32
}

impl Iterator for Counter {
    type Item = u32;
    
    fn next(&mut self) -> Option<u32> {
        self.count += 1;
        Some(self.count)
    }
}
```

### std::option

Option型ユーティリティ。

```restrict
use std::option::*;

let maybe = Some(42);

// 変換
maybe |> map(|x| x * 2);          // Some(84)
maybe |> filter(|x| x > 50);      // None
maybe |> flatMap(|x| Some(x + 1)); // Some(43)

// 値の抽出
maybe |> unwrap();                 // 42 (Noneの場合パニック)
maybe |> unwrapOr(0);             // 42
maybe |> unwrapOrElse(|| compute()); // 42

// チェーン
maybe
    |> map(|x| x.toString())
    |> orElse(|| Some("default"))
    |> unwrap();
```

### std::result

エラーハンドリングのためのResult型。

```restrict
use std::result::*;

let result: Result<i32, String> = Ok(42);

// 変換
result |> map(|x| x * 2);         // Ok(84)
result |> mapErr(|e| e.len());    // Ok(42)
result |> andThen(|x| Ok(x + 1)); // Ok(43)

// エラーハンドリング
result |> unwrap();                // 42 (Errの場合パニック)
result |> unwrapOr(0);            // 42
result |> unwrapOrElse(|e| handleError(e));

// パターンマッチング
match result {
    Ok(value) => value |> process,
    Err(error) => error |> logError
}

// Try演算子
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("ゼロ除算")
    } else {
        Ok(a / b)
    }
}

fn calculate() -> Result<i32, String> {
    let x = divide(10, 2)?;  // エラーの場合早期リターン
    let y = divide(x, 2)?;
    Ok(y)
}
```

### std::fs

ファイルシステム操作。

```restrict
use std::fs::*;

// ファイルの読み取り
let content = readFile("input.txt")?;
let bytes = readBytes("data.bin")?;

// ファイルの書き込み
"Hello, World!" |> writeFile("output.txt")?;
bytes |> writeBytes("output.bin")?;

// ファイル操作
exists("file.txt");           // bool
remove("temp.txt")?;
rename("old.txt", "new.txt")?;
copy("src.txt", "dst.txt")?;

// ディレクトリ操作
createDir("new_folder")?;
removeDir("old_folder")?;
readDir(".")?;  // エントリのイテレータ

// ファイルメタデータ
let meta = metadata("file.txt")?;
meta |> isFile();      // bool
meta |> isDir();       // bool
meta |> len();         // u64 (ファイルサイズ)
meta |> modified();    // DateTime
```

### std::time

時刻と日付の機能。

```restrict
use std::time::*;

// 現在時刻
let now = Instant::now();
let timestamp = SystemTime::now();

// 期間
let duration = Duration::fromSecs(60);
let elapsed = now.elapsed();

// フォーマット
timestamp |> formatRFC3339();  // "2024-01-15T10:30:00Z"

// スリープ
Duration::fromMillis(100) |> sleep;
```

### std::sync

同期プリミティブ（将来のマルチスレッドサポート用）。

```restrict
use std::sync::*;

// アトミック操作
let counter = AtomicU32::new(0);
counter |> fetchAdd(1);
counter |> load();

// Once（一度だけの初期化）
let INIT = Once::new();
INIT.callOnce(|| {
    // 一度だけ初期化
    setupGlobals();
});
```

### std::mem

メモリユーティリティ。

```restrict
use std::mem::*;

// サイズ情報
sizeOf::<i32>();      // 4
sizeOf::<String>();   // プラットフォーム依存

// メモリ操作
let mut x = 5;
let y = 10;
swap(&mut x, &mut y);  // x = 10, y = 5

// 所有権の取得
let value = take(&mut option);  // 移動して、Noneを残す
```

### std::convert

型変換トレイト。

```restrict
use std::convert::*;

// Intoトレイト
let string: String = "hello" |> into();
let number: i64 = 42i32 |> into();

// TryIntoトレイト
let small: i32 = 1000i64 |> tryInto()?;

// Fromトレイト実装
impl From<i32> for MyType {
    fn from(value: i32) -> Self {
        MyType { value }
    }
}
```

### std::hash

ハッシュユーティリティ。

```restrict
use std::hash::*;

// 値のハッシュ
let hash = "hello" |> hash();

// カスタムハッシュ可能型
#[derive(Hash)]
struct Point {
    x: i32,
    y: i32
}
```

### std::fmt

フォーマットと表示トレイト。

```restrict
use std::fmt::*;

// Displayトレイト
impl Display for Point {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

// Debugトレイト
#[derive(Debug)]
struct Complex {
    real: f64,
    imag: f64
}

// 使用法
point |> toString();     // Displayを使用
complex |> debug();      // Debugを使用
```

### std::math

数学関数。

```restrict
use std::math::*;

// 基本操作
abs(-5);         // 5
min(3, 7);       // 3
max(3, 7);       // 7
clamp(15, 0, 10); // 10

// 浮動小数点
3.14 |> floor();  // 3.0
3.14 |> ceil();   // 4.0
3.14 |> round();  // 3.0
16.0 |> sqrt();   // 4.0

// 三角関数
PI;              // 3.141592...
E;               // 2.718281...
45.0 |> toRadians() |> sin();
1.0 |> asin() |> toDegrees();

// べき乗と対数
2.0 |> pow(3.0);  // 8.0
100.0 |> log10(); // 2.0
E |> ln();        // 1.0
```

### std::random

乱数生成。

```restrict
use std::random::*;

// 乱数値
let mut rng = Rng::new();
rng |> nextU32();              // ランダムなu32
rng |> nextF64();              // [0, 1)のランダムなf64
rng |> range(1, 100);          // [1, 100)の範囲のランダム値

// ランダム選択
let items = vec!["a", "b", "c"];
items |> choose(&mut rng);      // ランダムな要素

// シャッフル
let mut numbers = vec![1, 2, 3, 4, 5];
numbers |>> shuffle(&mut rng);
```

### std::net

ネットワーク機能（非同期対応）。

```restrict
use std::net::*;

// TCP
let listener = TcpListener::bind("127.0.0.1:8080")?;
for stream in listener.incoming() {
    stream? |> handleClient;
}

// HTTPクライアント（簡略化）
let response = http::get("https://example.com")?;
response |> status();  // 200
response |> body();    // レスポンスコンテンツ
```

### std::env

環境変数とプログラム引数。

```restrict
use std::env::*;

// コマンドライン引数
let args = args();  // Vec<String>
let program = args[0];  // プログラム名

// 環境変数
let home = var("HOME")?;
setVar("MY_VAR", "value");
removeVar("OLD_VAR");

// 作業ディレクトリ
let cwd = currentDir()?;
setCurrentDir("/tmp")?;
```

### std::process

プロセス管理。

```restrict
use std::process::*;

// コマンドの実行
let output = Command::new("ls")
    |> arg("-la")
    |> output()?;

output |> stdout() |> toString();
output |> status() |> success();  // bool

// 終了
exit(0);  // 成功
exit(1);  // エラー
```

## 型トレイト

### Clone

```restrict
trait Clone {
    fn clone(&self) -> Self;
}

// 使用法
let original = MyType::new();
let copy = clone original;
```

### ToString

```restrict
trait ToString {
    fn toString(&self) -> String;
}

// 使用法
42 |> toString();  // "42"
```

### Default

```restrict
trait Default {
    fn default() -> Self;
}

// 使用法
let value = MyType::default();
```

### EqとOrd

```restrict
trait Eq {
    fn eq(&self, other: &Self) -> bool;
}

trait Ord {
    fn cmp(&self, other: &Self) -> Ordering;
}

// Derivable
#[derive(Eq, Ord)]
struct Point { x: i32, y: i32 }
```

## エラー型

標準ライブラリの一般的なエラー型：

```restrict
enum IoError {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    Other(String)
}

enum ParseError {
    InvalidFormat,
    Overflow,
    Empty
}
```

## ベストプラクティス

1. **失敗可能な操作にはResultを使用** - 不必要にパニックしない
2. **型推論を活用** - ただし明確性のために注釈を追加
3. **ループよりイテレータを優先** - より関数的で合成可能
4. **標準トレイトを使用** - 一貫性のためにClone、ToStringなど
5. **`with`でリソースを処理** - 自動クリーンアップ

## 例：ファイル処理

```restrict
use std::fs::*;
use std::io::*;

fn processFile(path: String) -> Result<(), IoError> {
    // ファイルを読む
    let content = path |> readFile()?;
    
    // 行を処理
    let processed = content
        |> lines()
        |> map(|line| line |> trim())
        |> filter(|line| !line.isEmpty())
        |> map(|line| line |> toUpperCase())
        |> collect::<Vec<_>>()
        |> join("\n");
    
    // 結果を書き込む
    processed |> writeFile("output.txt")?;
    
    Ok(())
}

fn main() {
    match "input.txt" |> processFile {
        Ok(()) => "ファイルが正常に処理されました" |> println,
        Err(e) => format!("エラー: {:?}", e) |> println
    }
}
```

標準ライブラリは、Restrict Languageのアフィン型システムとOSV構文とシームレスに動作するように設計されており、安全で人間工学的なプログラミング体験を提供します。