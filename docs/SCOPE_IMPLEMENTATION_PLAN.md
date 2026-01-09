# スコープ・ラムダ統合機能 実装計画

## 概要

ラムダ = パラメータ付きスコープという設計思想に基づき、スコープの合成・連結機能を段階的に実装する。

## 設計方針（再掲）

### 基本原則

1. **すべてのブロック `{}` はラムダとして作成される（遅延評価）**
2. **文脈によって自動実行されるか決まる**
   - 文（statement）位置: 作成して即座に実行 `{}()`
   - 式（expression）位置: 作成のみ、実行しない
   - OSV位置: ラムダを作成してオブジェクトに適用

3. **`it` キーワードで暗黙のパラメータを表現**
4. **スコープ合成 `+` で環境を結合**
5. **スコープ連結で値を渡す**

## Phase 1: `it` キーワードの実装

### 目標
暗黙のパラメータ `it` をサポートし、Kotlinスタイルのラムダを実現する。

### タスク

#### 1.1 Lexer の変更
- [ ] `it` を予約語として追加
- [ ] `Token::It` を定義
- [ ] テストケースの追加

**ファイル**: `src/lexer.rs`

```rust
pub enum Token {
    // ... existing tokens
    It,  // 'it' keyword
    // ...
}

// keyword 関数に追加
"it" => Token::It,
```

#### 1.2 AST の拡張
- [ ] `Expr::It` バリアントを追加
- [ ] ラムダ式に暗黙のパラメータフラグを追加

**ファイル**: `src/ast.rs`

```rust
pub enum Expr {
    // ... existing variants
    /// Implicit parameter reference
    It,
    // ...
}

pub struct LambdaExpr {
    pub params: Vec<String>,
    pub body: Box<Expr>,
    pub has_implicit_param: bool,  // NEW: it を使用するか
}
```

#### 1.3 Parser の変更
- [ ] `it` をパースして `Expr::It` を生成
- [ ] ラムダ式で `it` 使用を検出
- [ ] ブロック内での `it` 参照を許可

**ファイル**: `src/parser.rs`

```rust
fn primary_expr(input: &str) -> ParseResult<Expr> {
    alt((
        map(expect_token(Token::It), |_| Expr::It),
        // ... existing parsers
    ))(input)
}
```

#### 1.4 Type Checker の変更
- [ ] `Expr::It` の型チェック
- [ ] ラムダコンテキストで `it` が有効かチェック
- [ ] 外側のスコープで `it` が使われている場合にエラー

**ファイル**: `src/type_checker.rs`

```rust
// Lambda context tracking
struct TypeChecker {
    // ... existing fields
    in_lambda_with_it: bool,  // NEW: 暗黙のパラメータを持つラムダ内か
}

fn check_expr(&mut self, expr: &Expr) -> Result<TypedExpr, TypeError> {
    match expr {
        Expr::It => {
            if !self.in_lambda_with_it {
                return Err(TypeError::ItOutsideImplicitLambda);
            }
            // Return the lambda parameter type
            // ...
        }
        // ...
    }
}
```

#### 1.5 Code Generator の変更
- [ ] `Expr::It` のコード生成
- [ ] 暗黙のパラメータをローカル変数として扱う

**ファイル**: `src/codegen.rs`

```rust
fn generate_expr(&mut self, expr: &TypedExpr) -> String {
    match &expr.expr {
        Expr::It => {
            // Get local variable for 'it'
            self.get_local("it")
        }
        // ...
    }
}
```

#### 1.6 テスト
- [ ] `it` を使った単純なラムダのテスト
- [ ] OSV構文での `it` 使用テスト
- [ ] `it` が不正な場所で使われた場合のエラーテスト

**テストケース**:
```rust
// test_it_keyword.rl
fun main = {
    // Simple it usage
    val result = 5 |> |it| { it + 1 }

    // Implicit it (省略形、Phase 2 で対応)
    // val result2 = 5 |> { it + 1 }
}
```

---

## Phase 2: ブロックの遅延評価機構

### 目標
ブロックを遅延評価（ラムダ）として扱い、文脈に応じて自動実行する仕組みを実装。

### タスク

#### 2.1 AST の拡張
- [ ] ブロック式に評価モードフラグを追加
- [ ] `Expr::Block` と `Expr::Lambda` の区別を見直し

**設計案**:
```rust
pub enum Expr {
    // Option A: Block は常にラムダとして扱う
    Block(BlockExpr),  // これは実際には 0引数ラムダ

    // Option B: 明示的に区別
    Block {
        body: Vec<Expr>,
        is_lazy: bool,  // 遅延評価かどうか
    },
}
```

#### 2.2 Parser の変更
- [ ] ブロックの文脈を判定（statement vs expression）
- [ ] 文脈に応じて評価モードを設定

#### 2.3 Type Checker の変更
- [ ] 遅延評価ブロックの型を `() -> T` として扱う
- [ ] 即時評価ブロックの型を `T` として扱う

#### 2.4 Code Generator の変更
- [ ] 遅延評価ブロックをラムダとして生成
- [ ] 即時評価ブロックはラムダを生成して即座に呼び出し

#### 2.5 テスト
```rust
// test_lazy_blocks.rl
fun main = {
    // Eager (statement position)
    {
        print("executed immediately")
    }

    // Lazy (expression position)
    val deferred = {
        print("not executed yet")
        42
    }

    // Execute later
    deferred()  // prints "not executed yet", returns 42
}
```

---

## Phase 3: スコープ合成演算子 `+`

### 目標
`scopeA + scopeB` でスコープ（環境）を合成できるようにする。

### タスク

#### 3.1 AST の拡張
- [ ] `BinaryOp::ScopeCompose` を追加
- [ ] または `Expr::ScopeCompose` を別途定義

**設計案**:
```rust
pub enum Expr {
    // ... existing variants
    ScopeCompose {
        left: Box<Expr>,   // スコープA
        right: Box<Expr>,  // スコープB
    },
}
```

#### 3.2 Parser の変更
- [ ] `+` の文脈判定（算術 vs スコープ合成）
- [ ] スコープ同士の `+` をパース

**判定ロジック**:
- 左辺・右辺がブロック、Context、またはスコープ型 → スコープ合成
- それ以外 → 算術加算

#### 3.3 Type Checker の変更
- [ ] スコープ型の定義
- [ ] スコープ合成の型チェック
- [ ] 同名binding のチェック（エラー）

**型定義**:
```rust
pub enum TypedType {
    // ... existing types
    Scope {
        bindings: HashMap<String, TypedType>,
        result: Box<TypedType>,
    },
}
```

#### 3.4 Code Generator の変更
- [ ] スコープ合成のコード生成
- [ ] 両方のスコープの環境を結合

#### 3.5 テスト
```rust
// test_scope_composition.rl
fun main = {
    val scopeA = {
        val x = 10
        val y = 20
    }

    val scopeB = {
        val z = 30
    }

    val combined = scopeA + scopeB
    combined {
        print(x)  // 10
        print(y)  // 20
        print(z)  // 30
    }
}
```

---

## Phase 4: スコープ連結構文

### 目標
`scopeA scopeB` または `scopeA |> scopeB` でスコープを連結し、結果を渡せるようにする。

### タスク

#### 4.1 Parser の変更
- [ ] `scope { result } scope` 構文のパース
- [ ] 既存の `|>` との統合

#### 4.2 Type Checker の変更
- [ ] スコープ連結の型チェック
- [ ] 前のスコープの結果型と次のスコープのパラメータ型の整合性チェック

#### 4.3 Code Generator の変更
- [ ] スコープ連結のコード生成
- [ ] 結果の受け渡し

#### 4.4 テスト
```rust
// test_scope_concatenation.rl
fun main = {
    { 42 } { it + 1 }  // 43

    {
        val x = 10
        x * 2
    } {
        it + 5  // it = 20
    }  // 25
}
```

---

## Phase 5: Context as Scope の統合

### 目標
Context をスコープとして扱い、`with` を合成演算として統一する。

### タスク

#### 5.1 Context の型定義変更
- [ ] `ContextDecl` をスコープ型として扱う
- [ ] Context のインスタンスをスコープとして表現

#### 5.2 `with` の意味論変更
- [ ] `with ctx { }` を `ctx + { }` として扱う
- [ ] 複数 Context の合成をサポート

#### 5.3 Type Checker の変更
- [ ] Context のbindings をスコープ環境として扱う

#### 5.4 Code Generator の変更
- [ ] Context のメソッドをスコープ内で利用可能にする

#### 5.5 テスト
```rust
// test_context_as_scope.rl
context FileSystem {
    fun readFile: (String) -> String
    fun writeFile: (String, String) -> Unit
}

context Logger {
    fun log: (String) -> Unit
}

fun main = {
    with FileSystem + Logger {
        "Starting" |> log
        val data = "data.txt" |> readFile
        "output.txt" data writeFile
        "Complete" |> log
    }
}
```

---

## 実装順序の理由

1. **Phase 1 (`it`)**: 最も独立しており、他の機能に影響が少ない
2. **Phase 2 (遅延評価)**: `it` を活かすために必要な基盤
3. **Phase 3 (合成)**: 遅延評価の仕組みを使って実装
4. **Phase 4 (連結)**: 合成の仕組みを拡張
5. **Phase 5 (Context)**: すべての機能を統合

## 推定期間

- Phase 1: 2-3日
- Phase 2: 3-5日
- Phase 3: 2-3日
- Phase 4: 2-3日
- Phase 5: 1-2日

**合計**: 10-16日（集中作業の場合）

## 次のステップ

Phase 1 の実装から開始します。
