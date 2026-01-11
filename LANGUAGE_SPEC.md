# Restrict Language 仕様決定事項

このドキュメントは、Restrict Language の確定した言語仕様と実装方針をまとめたものです。

## 1. 基本構文仕様

### 1.1 キーワード
- **採用**: `fun`, `val`, `mut val`
- **廃止**: `fn`, `let`, `Unit` (型名)
- **理由**: パーサー実装との整合性、OSV哲学との一貫性

### 1.2 impl ブロック
- **採用しない**
- **代替**: Prototype + Freeze + @context + switcher パターン
- **理由**: クラス的階層の再導入を避け、型システムの複雑化を防ぐ

### 1.3 二項演算子
- **実装範囲**: 
  - 算術: `+`, `-`, `*`, `/`, `%`
  - 比較: `<`, `<=`, `>`, `>=`, `==`, `!=`
- **優先順位**: パイプ演算子 `|>` より高い
- **理由**: 標準ライブラリ実装に必須、最小限に抑制

### 1.4 条件分岐
- **採用構文**: `<boolean-expr> then { ... } else { ... }`
- **廃止**: `if ... else ...` 構文
- **理由**: OSV「値→動詞」の哲学に合わせた統一感

例:
```rust
val status = age >= 18 then { "adult" } else { "minor" }
```

### 1.5 Unit型表記
- **採用**: `()` 
- **廃止**: `Unit` キーワード
- **理由**: tuple-0 と同形で直感的、型と値の区別が明瞭

### 1.6 Arena構文
- **公式構文**: `with Arena { ... }`
- **非推奨**: `new_arena(...) { ... }`
- **理由**: lifetime ブロックと同形で学習コスト削減

### 1.7 論理演算子
- **追加**: `&&`, `||`, `!`
- **優先順位**: 算術演算子より低い
- **理由**: 一般的なコードで必須

### 1.8 パラメータ区切り
- **採用**: カンマ区切り
  - ラムダ: `|x, y|`
  - 関数: `fun add = x: Int32, y: Int32 { ... }`
- **理由**: スペース区切りは可読性・フォーマッタで曖昧

### 1.9 レコード記法
- **フィールド値**: `=` を使用
- **型注釈**: `:` を使用
- **理由**: Rust/Kotlin と同じ規則で混乱を減らす

例:
```rust
// レコード定義
record Point { x: Int32 y: Int32 }

// レコード作成
val p = Point { x = 10, y = 20 }
```

### 1.10 命名規則 (Naming Conventions)

Restrict Languageは **snake_case** を標準命名規則として採用する（OCaml/Rust風）。

#### 関数名・変数名
- **採用**: `snake_case`
- **理由**:
  - OSV構文での可読性（`(s) string_length` vs `(s) stringLength`）
  - Rust（コンパイラ実装言語）との親和性
  - 関数型言語（OCaml, Haskell）の慣例に準拠

```rust
// 良い例
val user_name = "Alice"
fun string_length: (s: String) -> Int = { ... }
fun is_digit: (c: Char) -> Bool = { ... }
fun char_to_int: (c: Char) -> Int = { ... }

// 避けるべき例
val userName = "Alice"      // camelCase は非推奨
fun stringLength = { ... }  // camelCase は非推奨
```

#### 型名・レコード名
- **採用**: `PascalCase`
- **理由**: 値と型を視覚的に区別

```rust
// 型名は PascalCase
record UserProfile { name: String, age: Int32 }
enum HttpStatus { Ok, NotFound, ServerError }

// ジェネリック型パラメータは大文字1文字
fun map<T, U>: (list: List<T>, f: |T| -> U) -> List<U> = { ... }
```

#### 定数
- **採用**: `SCREAMING_SNAKE_CASE`（オプション）
- **理由**: 定数と変数を視覚的に区別

```rust
val MAX_BUFFER_SIZE = 1024
val DEFAULT_TIMEOUT = 30
```

#### まとめ

| 種類 | 規則 | 例 |
|------|------|-----|
| 関数名 | `snake_case` | `string_length`, `to_upper` |
| 変数名 | `snake_case` | `user_name`, `total_count` |
| 型名 | `PascalCase` | `UserProfile`, `HttpResponse` |
| レコード名 | `PascalCase` | `Point`, `Rectangle` |
| 型パラメータ | 大文字1文字 | `T`, `U`, `E` |
| 定数（任意） | `SCREAMING_SNAKE_CASE` | `MAX_SIZE` |

## 2. 型システム

### 2.1 基本型
- `Int32`, `Float64`, `String`, `Char`, `Boolean`, `()`
- ジェネリック型: `List<T>`, `Option<T>`, `Array<T, N>`
- 時相型: `Type<~t>` with constraints

### 2.2 Result型
- **採用**: `Result<T, E>`
- **初期実装**: `match` ベースのエラーハンドリング
- **将来**: `?` 演算子は予約のみ

### 2.3 Temporal Affine Types (TAT)
- **構文**: `where ~tx within ~db`
- **用途**: リソースの生存期間管理

## 3. 関数とラムダ

### 3.1 関数定義
```rust
// 標準形：パラメータと戻り値型を明示
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

// 戻り値型は推論可能な場合省略可
fun double: (x: Int32) = {
    x * 2
}

// パラメータなし
fun getAnswer: () -> Int32 = {
    42
}

// ジェネリクスと時相パラメータ
fun identity: <T>(value: T) -> T = {
    value
}

fun process: <~t>(data: Data<~t>) -> Result<Data<~t>, Error> = {
    data |> validate |> transform
}
```

### 3.2 OSV構文
```rust
// 多引数
val result = (10, 20) add

// 単一引数（パイプ）
val doubled = 42 |> double
```

### 3.3 パイプ演算子
- `|>`: 不変パイプ（単一の標準パイプ演算子）

## 4. パターンマッチング

### 4.1 exhaustiveness
- **必須**: non-exhaustive パターンは型エラー
- **理由**: 安全性の基本要件

### 4.2 パターン種類
```rust
// Option
x match {
    Some(v) => { v * 2 }
    None => { 0 }
}

// List
list match {
    [] => { "empty" }
    [head | tail] => { head }
    [a, b] => { a + b }
    _ => { "other" }
}

// Record
point match {
    Point { x = 0, y = 0 } => { "origin" }
    Point { x, y } => { x + y }
}
```

## 5. リソース管理

### 5.1 環境スコープの統一構文
すべての環境は `X { ... }` の形式で「Xが浸透した環境」を表現：

1. `temporal ~t { ... }` - 時相環境（~tが存在）
2. `AsyncRuntime { ... }` - 非同期実行環境
3. `Arena { ... }` - メモリアリーナ環境
4. `Database { ... }` - データベースコンテキスト環境

```rust
// 環境の入れ子
Database {
    temporal ~db {
        val conn = connect<~db>()
        // Database環境かつ~db時相が利用可能
    }
}
```

### 5.2 Clone/Freeze
```rust
// Clone with modification
val newObj = obj.clone { field = newValue }

// Freeze (immutable copy)
val frozen = obj.freeze
```

## 6. 非同期プログラミング

### 6.1 async/await
```rust
with AsyncRuntime<~async> {
    val task = 21 |> compute |> spawn;
    val result = task |> await
}
```

## 7. モジュールシステム

### 7.1 基本方針
- **採用**: Go-like「1ディレクトリ = 1パッケージ」
- **将来**: alias/re-export はβフェーズで検討

## 8. 実装優先順位

### Phase 1 (現在)
1. 基本構文の安定化
2. 型システムの完成
3. パターンマッチングの完全実装
4. TAT (Temporal Affine Types) の安定化

### Phase 2
1. Result型とエラーハンドリング
2. 標準ライブラリの拡充
3. モジュールシステムの完成
4. 最適化

### Phase 3
1. 文字列補間
2. `?` 演算子
3. マクロシステム（検討）
4. SIMD/WebGPU対応

## 9. 破壊的変更の方針

このドキュメントに記載された仕様は、v1.0 リリースまでは変更される可能性があります。
ただし、以下の項目は確定とし、変更しません：

- OSV構文
- アフィン型システム
- `fun`/`val` キーワード
- パイプ演算子 `|>`, `|>>`
- 基本的なパターンマッチング構文

## 10. 追加決定事項

### 10.1 文字列処理
- **文字列連結**: `+` 演算子は数値同士または文字列同士のみ
- **暗黙型変換**: 一切禁止
- **明示的変換**: `toString()` または `format()` を使用

例:
```rust
// OK
val s1 = "Hello" + " World"
val n = 1 + 2

// エラー
val s2 = 1 + "2"  // 型エラー

// 正しい書き方
val s3 = 1.toString() + "2"
val s4 = format "{} {}" 1 "2"
```

### 10.2 配列とリスト
- **統一リテラル**: `[a, b, c]` のみ使用
- **廃止**: `[|a, b, c|]` 記法
- **型による区別**: 
  - `Array<T, N>`: 固定長配列
  - `List<T>`: 可変長リスト

### 10.3 繰り返し構造
- **while構文**: `condition while { ... }` (OSV順)
- **廃止**: `while condition { ... }` および `for` ループ
- **代替**: 再帰、`map`、`filter`、`fold`

例:
```rust
// OSV while
(count < 10) while {
    count = count + 1
}

// 関数型スタイル（推奨）
[0..10] |> map |x| x * 2
```

### 10.4 Unicode対応
- **文字列エンコーディング**: UTF-8必須
- **文字列操作**: Code-pointベース
- **インデックス**: バイト位置ではなく文字位置

### 10.5 数値リテラル拡張
- **区切り文字**: `1_000_000`
- **16進数**: `0xFF`、`0x1A2B`
- **指数表記**: `1.5e10`、`3.14E-2`

### 10.6 エラーハンドリング詳細
- **Arena満杯**: `Result<T, OutOfMemory>` を返却
- **panic代替**: `fatal()` 関数で明示的終了
- **unwrap代替**: `expectOr(default)` パターン

### 10.7 FFI (Foreign Function Interface)
- **初期サポート**: WASM import/export のみ
- **将来計画**: ネイティブFFIはv2.0以降

### 10.8 最適化
- **定数畳み込み**: コンパイル時評価（整数、浮動小数点、文字列連結）
- **デッドコード除去**: 未使用関数の自動削除
- **インライン化**: 小規模関数の自動インライン

### 10.9 廃止される構文
以下の構文は正式に廃止：
- `[|...|]` 配列リテラル
- `|>>` パイプ演算子（削除決定）
- `while condition { ... }` 前置while
- `if ... else ...` 構文
- `Unit` 型名キーワード
- 暗黙の型変換

## 11. 追加仕様決定事項

### 11.1 Range型とリテラル
- **Range型**: `Range<T, ~t>` - 時相統合された範囲型
- **構文**: `[start..end]` はRange型を生成（Arrayではない）
- **使用例**:
```rust
val range = [0..10]  // Range<Int32, ~local>
val list = range |> toList  // List<Int32>への変換
val array = range |> toArray<10>  // Array<Int32, 10>への変換
```

### 11.2 オーバーフロー処理
- **全ての算術演算**: `Result<T, Overflow>`を返す
- **`?`演算子（Phase 2）**: OSV構文 `expr ? handle`
  - `handle`はエラーハンドラ（ラムダ可）
  - 例: `(x + y) ? |err| 0`  // オーバーフロー時は0を返す
- **fatal関数**: 時相クリーンアップを自動実行してから終了
```rust
fatal "Critical error: {msg}"  // 全ての時相リソースを解放後に終了
```

### 11.3 標準ライブラリ命名規則
- **関数名**: camelCase（例: `toString`, `parseInt`）
- **型名**: PascalCase（例: `String`, `Option`）
- **定数**: SCREAMING_SNAKE_CASE（例: `MAX_INT`）
- **メソッド vs 関数**: コンテキストに応じて決定
  - データに密着: メソッド（例: `list.map`）
  - 汎用操作: 関数（例: `parseInt`）

### 11.4 モジュールシステム
- **import構文**: `import "path/to/module" as alias`
- **可視性**: `pub`修飾子のみ（デフォルトは非公開）
- **循環依存**: ランタイム検知で`fatal`
- **名前衝突解決**: 時相名前空間 `mod<~t>::func`
```rust
import "std/io" as io
import "std/math" as math

pub fun calculate = x: Int32 {
    x |> math::sqrt |> io::println
}
```

### 11.5 エラー処理システム
- **標準エラー型**: `enum Error<~t> { ... }` - 時相付きエラー
- **エラー伝播**: `match`必須、`?`で時相エラー伝播
- **`?`構文詳細**: `expr ? |err| handle(err)`
```rust
enum Error<~t> {
    Overflow<~t>
    OutOfBounds<~t> { index: Int32, size: Int32 }
    IO<~t> { message: String }
    Custom<~t> { message: String }
}

fun safeDivide = x: Int32, y: Int32 -> Result<Int32, Error<~local>> {
    (y == 0) then {
        Err(Error::Custom { message: "Division by zero" })
    } else {
        Ok(x / y)
    }
}

// 使用例
val result = (10, 0) |> safeDivide ? |err| {
    err match {
        Error::Custom { message } => { 0 }  // デフォルト値
        _ => { fatal "Unexpected error" }
    }
}
```

### 11.6 ビルドシステム
- **マニフェスト**: シンプルなTOML形式
- **バージョン管理**: Git hashベース（実験的リリース向き）
```toml
[package]
name = "my-app"
version = "git:a1b2c3d"  # Git commit hash

[dependencies]
std = "git:main"  # 標準ライブラリ
http = { git = "https://github.com/restrict/http", rev = "e4f5g6h" }
```

### 11.7 数値型拡張
- **追加整数型**: `Int8`, `Int16`, `Int64`
- **符号なし型（実験的）**: `UInt8`, `UInt16`, `UInt32`, `UInt64`
- **オーバーフロー動作**: 全て`Result<T, Overflow<~t>>`でラップ
- **チェック済み演算**: `+?`, `-?`, `*?` など
```rust
val x: Int8 = 127
val y: Int8 = 1
val sum = (x +? y) ? |_| Int8::MAX  // オーバーフロー時は最大値
```

### 11.8 デバッグ機能
- **ドキュメントコメント**: `///` （3スラッシュ）
- **インラインドキュメント**: `//!` （モジュール/関数内部）
- **REPL**: TATシミュレーション付き対話環境
- **デバッグ出力**: `debug!` マクロ（Phase 3）
```rust
/// 二つの数値を加算する
/// 
/// # Examples
/// ```
/// val result = (5, 10) |> add
/// ```
fun add = x: Int32, y: Int32 -> Int32 {
    //! 内部実装の説明
    x + y
}
```

### 11.9 テンポラル型詳細
- **時相変数宣言**: 将来の設計課題として保留
- **時相推論**: スコープベースの自動推定
- **明示的時相**: `<~t>`構文での指定
```rust
// 時相推論の例
with lifetime<~db> {
    val conn = Database::connect()  // 自動的に~db時相
    val data = conn |> query  // dataも~db時相を継承
}
```

### 11.10 非同期プログラミング詳細
- **async関数構文**: `fun name<~t> = async { ... }`
- **キャンセル機構**: `task.cancel<~t>`
- **エラー処理**: TAT統合`Result<T, Error<~async>>`
```rust
fun fetchData<~async> = url: String -> Result<String, Error<~async>> async {
    val response = url |> http::get |> await
    response.status match {
        200 => { Ok(response.body) }
        _ => { Err(Error::IO { message: "Failed to fetch" }) }
    }
}

with AsyncRuntime<~async> {
    val task = "https://api.example.com" |> fetchData |> spawn
    
    // キャンセル可能
    condition then {
        task.cancel<~async>
    } else {
        task |> await ? |err| ""
    }
}
```

### 11.11 コンテキストバインディング (@context)

コンテキストバインドにより、特定のコンテキストを持つスコープからのみ呼び出し可能な関数を定義できます。

#### コンテキスト定義
```rust
context Database<~t> {
    connection: Connection<~t>
    timeout: Int32
}

context Transactional<~t> {
    datasource: Datasource<~t>
    connection: Connection<~t>
}
```

#### コンテキストバインド関数
```rust
// @Databaseを前置 - Databaseコンテキスト内でのみ呼び出し可能
@Database
fun query: (sql: String) -> Result<Data, Error> = {
    // connection は暗黙的に利用可能
    connection |> execute sql
}

// 複数コンテキストの要求
@Transactional
@Database
fun save: (entity: Entity) -> Result<(), Error> = {
    // datasource, connection が暗黙的に利用可能
    entity |> validate then {
        connection |> insert entity
    } else {
        Err(ValidationError)
    }
}
```

#### コンテキスト環境の作成
```rust
// コンテキスト環境の作成（withキーワードなし）
Database { connection = conn, timeout = 5000 } {
    "SELECT * FROM users" |> query  // OK: Database環境内
    users |> map |user| {
        user.id |> findDetails  // OK: ネストしたスコープでも有効
    }
}

// コンテキスト外からの呼び出しはコンパイルエラー
"SELECT * FROM users" |> query  // エラー: Database環境が必要
```

#### 時相型との統合
```rust
context AsyncDatabase {
    maxConnections: Int32
}

@AsyncDatabase
fun asyncQuery: <~t>(sql: String) -> Task<Data<~t>> = {
    getConnection() |> spawn |conn| {
        conn |> execute sql
    }
}

// 使用例
AsyncRuntime {
    temporal ~async {
        AsyncDatabase { maxConnections = 10 } {
            val task = "SELECT * FROM orders" |> asyncQuery<~async>
            task |> await
        }
    }
}
```

#### コンテキストの合成
```rust
// 複数のコンテキストを要求する関数
@Database
@Transactional
fun transactionalSave: (record: Record) -> Result<(), Error> = {
    // Database と Transactional 両方のフィールドにアクセス可能
    connection |> beginTransaction
    record |> save
    connection |> commit
}

// ネストした環境で複数コンテキストを提供
Database { connection = conn, timeout = 5000 } {
    Transactional { datasource = ds, connection = conn } {
        newRecord |> transactionalSave
    }
}
```

#### 設計原則
1. **明示的な依存**: 関数が必要とするコンテキストを型レベルで宣言
2. **スコープ制限**: コンテキスト外からの不正な呼び出しを防止
3. **暗黙的アクセス**: コンテキスト内のフィールドは暗黙的に利用可能
4. **時相統合**: TAT（Temporal Affine Types）と自然に統合
5. **合成可能**: 複数のコンテキストを組み合わせて使用可能

### 11.12 時相の明示的管理

#### 時相変数の作成
```rust
// コンテキスト内での時相作成
Database {
    temporal ~db  // 明示的に時相変数を定義
    
    val conn = connect<~db>()
    val data = query<~db>(conn, "SELECT...")
}

// 純粹な時相スコープ
temporal ~session {
    val data = SessionData<~session> { id = 123 }
}

// 時相制約
Database {
    temporal ~db
    
    Transaction {
        temporal ~tx where ~tx within ~db
        
        performTransaction<~tx, ~db>()
    }
}
```

#### 関数での時相使用
```rust
// 時相パラメータを持つ関数（常に明示的）
@Database
fun connect: <~t>() -> Connection<~t> = {
    Connection<~t> { id = generateId() }
}

@Database
fun query: <~t>(conn: Connection<~t>, sql: String) -> Result<Data<~t>, Error> = {
    conn |> executeInternal sql
}

// 複数の時相と制約
@Database
@Transaction
fun transactionalQuery: <~tx, ~db>(
    conn: Connection<~db>, 
    sql: String
) -> Result<Data<~tx>, Error> 
where ~tx within ~db = {
    // 実装
}
```

### 11.13 完全な例：Webアプリケーション

```rust
// コンテキスト定義
context Server {
    port: Int32
}

context Database {
    host: String
}

context Logger {
    level: LogLevel
}

// 関数定義
@Server
fun handle: (path: String, handler: |Request| -> Response) -> () = {
    registerHandler(path, handler)
}

@Database
fun getUser: <~t>(userId: Int32) -> Option<User<~t>> = {
    temporal ~query
    query<~query>("SELECT * FROM users WHERE id = ?", userId)
        |> parseUser<~t>
}

@Logger
fun log: (level: LogLevel, message: String) -> () = {
    formatLog(level, message) |> writeToFile
}

// アプリケーション
fun main: () = {
    Server { port = 8080 } {
        Logger { level = Info } {
            Database { host = "localhost" } {
                temporal ~app
                
                handle("/users/:id", |req| {
                    temporal ~request where ~request within ~app
                    
                    val userId = req.params.id |> parseInt
                    userId match {
                        Ok(id) => {
                            getUser<~request>(id) match {
                                Some(user) => {
                                    log(Info, "User found: " + user.name)
                                    Response { status = 200, body = user |> toJson }
                                }
                                None => {
                                    log(Warning, "User not found: " + id)
                                    Response { status = 404, body = "Not found" }
                                }
                            }
                        }
                        Err(_) => {
                            Response { status = 400, body = "Invalid user ID" }
                        }
                    }
                })
                
                log(Info, "Server started on port 8080")
                serve()  // サーバー起動
            }
        }
    }
}
```

---

最終更新: 2024年12月
