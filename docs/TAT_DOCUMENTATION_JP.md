# Temporal Affine Types (TAT) 仕様書

## 概要

Temporal Affine Types（TAT）は、Restrict言語における時間的リソース管理システムです。このシステムは、アフィン型と時間制約を組み合わせることで、メモリ安全性とリソース管理を保証します。

## 主要特徴

### 1. 時間的ライフタイム（Temporal Lifetimes）

時間的ライフタイム（`~lifetime`）は、リソースの有効期間を表します。

```rust
// 基本的な時間的ライフタイム
record File<~f> {
    path: String,
    content: String
}

fun main: () = {
    with lifetime<~f> {
        val file = File { path = "test.txt", content = "data" };
        file.content  // ~f スコープ内でのみ有効
    }
    // ~f スコープを抜けると file は無効
}
```

### 2. 時間制約（Temporal Constraints）

時間制約は、ライフタイム間の関係を定義します。

```rust
record Database<~db> {
    name: String,
    connection: String
}

record Transaction<~tx, ~db> where ~tx within ~db {
    id: Int32,
    db: Database<~db>
}

fun main: () = {
    with lifetime<~db> {
        with lifetime<~tx> where ~tx within ~db {
            val db = Database { name = "mydb", connection = "localhost" };
            val tx = Transaction { id = 1, db = db };
            tx.id
        }
    }
}
```

### 3. 非同期プログラミング（Async Programming）

TATは非同期プログラミングと統合されています。

```rust
record Task<T, ~async> {
    id: Int32
}

fun main: () = {
    with lifetime<~async> {
        with AsyncRuntime<~async> {
            val task = spawn { User { id = 42, name = "Test" } };
            val user = await task;
            user.id
        }
    }
}
```

## 実装詳細

### 1. 型チェッカー統合

TATは型チェッカー（`TypeChecker`）に統合されています：

- **時間的コンテキスト管理**: `temporal_contexts` フィールドでライフタイムスコープを管理
- **制約検証**: `TemporalConstraint` 構造体で時間制約を表現
- **AsyncRuntime 統合**: `async_runtime_stack` で非同期コンテキストを管理

### 2. 主要データ構造

#### TemporalConstraint
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TemporalConstraint {
    pub inner: String,    // 内側のライフタイム
    pub outer: String,    // 外側のライフタイム
}
```

#### AsyncRuntime コンテキスト
```rust
// AsyncRuntime 管理メソッド
fn enter_async_runtime(&mut self, lifetime: &str) -> Result<(), TypeError>
fn exit_async_runtime(&mut self) -> Result<String, TypeError>
fn current_async_runtime(&self) -> Option<&String>
```

### 3. 構文サポート

#### ライフタイム宣言
```rust
with lifetime<~name> {
    // ~name スコープ内のコード
}
```

#### 時間制約付きライフタイム
```rust
with lifetime<~inner> where ~inner within ~outer {
    // 制約付きコード
}
```

#### AsyncRuntime コンテキスト
```rust
with AsyncRuntime<~async> {
    val task = spawn { computation };
    val result = await task;
}
```

## 使用例

### 1. 基本的なファイル操作

```rust
record File<~f> {
    path: String,
    content: String
}

fun readFile<~f> = path: String -> File<~f> {
    File { path = path, content = "file content" }
}

fun main: () = {
    with lifetime<~f> {
        val file = readFile("data.txt");
        file.content
    }
}
```

### 2. データベース操作

```rust
record Database<~db> {
    name: String,
    connection: String
}

record Transaction<~tx, ~db> where ~tx within ~db {
    id: Int32,
    db: Database<~db>
}

record Query<~q, ~tx, ~db> where ~q within ~tx, ~tx within ~db {
    sql: String,
    tx: Transaction<~tx, ~db>
}

fun main: () = {
    with lifetime<~db> {
        with lifetime<~tx> where ~tx within ~db {
            with lifetime<~q> where ~q within ~tx {
                val db = Database { name = "mydb", connection = "localhost" };
                val tx = Transaction { id = 1, db = db };
                val query = Query { sql = "SELECT * FROM users", tx = tx };
                query.sql
            }
        }
    }
}
```

### 3. 非同期処理

```rust
record Task<T, ~async> {
    id: Int32
}

record User {
    id: Int32,
    name: String
}

fun main: () = {
    with lifetime<~async> {
        with AsyncRuntime<~async> {
            // 複数のタスクを並行実行
            val task1 = spawn { User { id = 1, name = "Alice" } };
            val task2 = spawn { User { id = 2, name = "Bob" } };
            
            // 結果を待機
            val user1 = await task1;
            val user2 = await task2;
            
            user1.id + user2.id
        }
    }
}
```

### 4. 時間的制約とasyncの統合

```rust
record File<~f> {
    path: String,
    content: String
}

record AsyncFile<~f, ~async> where ~f within ~async {
    file: File<~f>,
    status: String
}

fun main: () = {
    with lifetime<~async> {
        with lifetime<~f> where ~f within ~async {
            with AsyncRuntime<~async> {
                val task = spawn { 
                    AsyncFile { 
                        file = File { path = "async.txt", content = "async data" },
                        status = "ready"
                    } 
                };
                val async_file = await task;
                async_file.file.content
            }
        }
    }
}
```

## 制約とルール

### 1. 時間制約ルール

- **包含関係**: `~inner within ~outer` は `~inner` が `~outer` のスコープ内でのみ有効であることを意味
- **推移性**: `~a within ~b` かつ `~b within ~c` なら `~a within ~c`
- **順序**: 外側のライフタイムが先に宣言される必要がある

### 2. AsyncRuntime ルール

- **コンテキスト必須**: `spawn` と `await` は `AsyncRuntime` コンテキスト内でのみ使用可能
- **ライフタイム対応**: `AsyncRuntime<~async>` は対応するライフタイムを持つ必要がある
- **ネスト可能**: AsyncRuntime コンテキストはネストできる

### 3. アフィン型ルール

- **単一使用**: 各バインディングは最大1回まで使用可能
- **移動セマンティクス**: 値の使用は所有権の移動を伴う
- **スコープ終了**: ライフタイムスコープ終了時に自動的にリソースが解放

## エラーハンドリング

### 1. 時間制約違反

```rust
// エラー例：逆順の制約
record Transaction<~tx, ~db> where ~tx within ~db {
    id: Int32
}

fun main: () = {
    with lifetime<~tx> {
        with lifetime<~db> where ~db within ~tx {  // エラー！
            val tx = Transaction { id = 1 };
            tx.id
        }
    }
}
```

### 2. AsyncRuntime コンテキストエラー

```rust
// エラー例：AsyncRuntime コンテキストなしでspawn
fun main: () = {
    with lifetime<~async> {
        val task = spawn { 42 };  // エラー！AsyncRuntime コンテキストが必要
        await task
    }
}
```

### 3. ライフタイム不一致

```rust
// エラー例：未定義のライフタイム
fun main: () = {
    with lifetime<~valid> {
        with AsyncRuntime<~invalid> {  // エラー！~invalid は未定義
            val task = spawn { 42 };
            await task
        }
    }
}
```

## 実装状況

### 完了済み機能

✅ 基本的な時間的ライフタイム
✅ 時間制約（within関係）
✅ AsyncRuntime コンテキスト
✅ spawn/await 操作
✅ Task<T, ~async> 型
✅ 包括的なテストスイート

### 進行中の機能

🔄 アリーナベースメモリ管理
🔄 自動クリーンアップコード生成
🔄 より詳細な時間制約検証

### 予定機能

📋 時間的チャネル（temporal channels）
📋 より高度な並行制御
📋 パフォーマンス最適化

## 技術的詳細

### 1. メモリ管理

TATは以下のメモリ管理戦略を使用します：

- **スタックベース**: 基本的な値はスタックに格納
- **アリーナ割り当て**: 時間的スコープごとにアリーナを使用
- **自動クリーンアップ**: スコープ終了時に自動的にリソースを解放

### 2. WASM統合

TATはWebAssemblyとの統合を考慮して設計されています：

- **GCフリー**: ガベージコレクション不要
- **予測可能性**: 決定論的なメモリ管理
- **効率性**: 最小限のランタイムオーバーヘッド

### 3. 型推論

TATは型推論システムと統合されています：

- **双方向型チェック**: 型の推論と検証
- **時間制約推論**: 自動的な制約推論
- **エラー報告**: 明確なエラーメッセージ

## 今後の展開

### 1. 短期目標

- アリーナベースメモリ管理の完全実装
- 自動クリーンアップコード生成
- パフォーマンス最適化

### 2. 中期目標

- 時間的チャネルの実装
- より高度な並行制御プリミティブ
- 実用的なアプリケーション例

### 3. 長期目標

- 他言語との相互運用性
- 標準ライブラリの拡張
- 産業利用への適用

## 結論

Temporal Affine Types（TAT）は、Restrict言語における革新的なリソース管理システムです。時間的制約とアフィン型を組み合わせることで、メモリ安全性と効率性を両立させています。非同期プログラミングとの統合により、現代的なアプリケーション開発のニーズに応える設計となっています。

このシステムは、従来のガベージコレクションに依存しない新しいメモリ管理パラダイムを提供し、WebAssemblyなどの制約環境での高性能アプリケーション開発を可能にします。