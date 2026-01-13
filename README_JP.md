<div align="center">
  <img src="assets/logo.svg" alt="Restrict Language Logo" width="200" height="200">
  
  # Restrict Language
  
  **A functional programming language with affine types for WebAssembly**
  
  [![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
  [![WASM](https://img.shields.io/badge/target-WebAssembly-orange.svg)](https://webassembly.org/)
</div>

---

## 仕様書

*2025-07-10*

---

### 0. 目次

1. コア概念 & 所有権モデル
2. 型と値
3. データ定義 - *record / clone / freeze*
4. 変数束縛 & パイプ列
5. 語順・演算子・関数
6. 制御構文（後置動詞）
7. コンテキスト束縛 & with 句
8. 標準リソースブロック ― *Arena*
9. ランタイム & コンパイルパス
10. 予約語一覧
11. BNF 定義
12. 今後拡張フック

---

### 0. 設計理念 & 想定用途

| 指針                 | 詳細                                                                                                                                     |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| **① 迷いを削る“制限主義”**  | ・語順は OSV（Object-Subject-Verb）後置を原則。<br>・変数は **アフィン**（参照 0-1 回）。<br>・派生は **clone + freeze** で差分だけ許す。<br>→「どこで何が呼ばれ、何が共有されるか」を 10 秒で追える。 |
| **② データフローの可視化**   | パイプ演算子 \`                                                                                                                              | >`＋ 束縛糖衣で **左→右** に値が流れる。<br>副作用はコンテキスト or`async then\` に押し込める。 |
| **③ 静的安全 × 手軽さ**   | 値コピー禁止領域は型が守り、`clone` は明示。<br>GC なし／Arena ブロックで確実解放。                                                                                   |
| **④ WASM ファースト**   | *ブラウザ*・*Cloudflare Workers*・*WASI* で同じバイナリを動かす。<br>WASM MVP + threads + (将来) SIMD/GPU backend が標準ターゲット。                                |
| **⑤ Web ↔ ゲーム 両立** | ▸ **Web/Serverless**: コンテキスト束縛で安全に外部サービス。</br>▸ **ゲーム/リアルタイム**: Prototype+Freeze でデザイ


## 🚀 クイックスタート

### Warder（パッケージマネージャー）を使用

```bash
# warderを使って新しいプロジェクトを作成
warder new my-project
cd my-project

# ビルドして実行
warder build
warder run
```

### 基本的な例

#### Hello World
```rust
// hello.rl
fun main = {
    "Hello, Restrict Language!" |> println
}
```

#### 算術演算
```rust
// arithmetic.rl
fun add = x:Int y:Int {
    x + y
}

fun main = {
    val result = (10, 20) add
    "Result: " |> println
    result |> print_int
}
```

---

## 1. コア概念

| 概念            | 要旨                                         |
| ------------- | ------------------------------------------ |
| **値**         | 不変データ: リテラル / record / 関数 / コンテキスト / ブロック値 |
| **参照**        | 値の場所ラベル。型・寿命を静的解析                          |
| **束縛 (Bind)** | `Bind{name,value,mut?}` — 名前と値を 1 回だけ結合    |
| **アフィン**      | 各束縛は 0-1 回参照。複製は `clone` 必須                |
| **スコープ**      | レキシカル `{}` / パイプ列 / コンテキスト stack           |

---

## 2. 型と値

- *基礎型* `Int32 Float64 Boolean String Char Unit`
- *複合* `Option<T> Tuple<A,B> List<T> Array<T,N> Function`
- *ユーザ* `record` / `context` インスタンス

---

## 3. データ定義 ― Prototype + Freeze

```ocaml
record Enemy { hp: Int, atk: Int }

val base  = Enemy { hp = 100, atk = 10 }    // open
val boss  = base.clone { hp = 500 } freeze  // closed
```

* open record ⇒ `clone` で差分更新
* `freeze` で closed record 型へ確定（以降フィールド追加不可）

`impl` でメソッド追加（仮想・継承なし）

```ocaml
impl Enemy {
    fun attack = self: Enemy tgt: Player { tgt.damage self.atk }
}
boss luke.attack
```

---

## 4. 変数束縛

| 記法 | 等価展開         | 役割                               | 使用例                     |
| --------------- | ------------- | ---------- | ------ |
| `val x = e`     | `e val x`     | 宣言（不変）     | `val x = 5`       |
| `mut val x = e` | `e mut val x` | 宣言（可変）     | `mut val x = 5`   |
| `x = e`         |               | 再代入（要mut）  | `x = x + 1`       |
| `e \|>  x`   | `val x = e`     | パイプ束縛（不変）                 | `fetch "/api" |> raw`      |
| `e \|>> x`   | `mut val x = e` | パイプ束縛（可変）         | `0 |>> counter`            |

**セミコロンフリー構文**: Kotlin スタイルの改行ベースの文終端を採用しています。セミコロンは省略可能です：
```rl
// 改行で文を区切る（セミコロン不要）
mut val x = 5
x = x + 1
x

// 1行に複数の文を書く場合はセミコロンが必要
val a = 1; val b = 2; a + b

// 演算子の後の改行は継続行として扱われる
val sum = 10 +
    20 +
    30
```

---

## 4.1 ジェネリクス

型パラメータを持つ関数とレコードを定義できます：

```rust
// ジェネリック関数（型パラメータは関数名の後）
fun identity<T>: (x: T) -> T = {
    x
}

// ジェネリックレコード
record Box<T> {
    value: T
}

record Pair<A, B> {
    first: A,
    second: B
}

// 使用時に型が推論される
val a = 42 identity          // T = Int
val b = "hello" identity     // T = String
val box = Box { value = 42 } // Box<Int>
```

パターンマッチでレコードを分解：
```rust
fun swap<A, B>: (p: Pair<A, B>) -> Pair<B, A> = {
    p match {
        Pair { first, second } => { Pair { first = second, second = first } }
    }
}
```

---

## 5. 語順・演算子・関数

* **OSV**: `obj subj.verb`
* **インフィックス例外**: `+ - * / % == != < <= > >=`
* **複数引数**: `(a,b,c) func`
* **パイプ**: `expr |> name`（識別子なら束縛）

関数は 1 引数カリー。

```rust
fun add = a:Int b:Int { a + b }
```

---

## 6. 制御構文（後置動詞）

```ocaml
cond then { … }                     // if
else cond2 then { … } … else { … }

cond while { … }                    // loop

expr match {
    Pat1 => { … }
    _    => { … }
}
```

各ブロックは式・型一致必須。

---

## 7. コンテキスト束縛 & **with 句**

```ocaml
context Web { val db: JsonStore }

fun getUser = @Web id:Int { ("users",id) Web.db.select }

with Web {                     // コンテキストを push
    42 getUser |> u
}                              // pop  → 以降呼べない
```

* コンパイラがブロック範囲で @Ctx 参照を静的許可。
* `with (Ctx1, Ctx2) { … }` ― 複数 push も可（右端から pop）。

---

## 8. 標準リソースブロック ― Arena

```ocaml
with Arena {                   // 高速一括 free
    val tex = newTexture bytes
}
```

Arena は `context Arena` として定義され、ブロック終端で確実に解放。

---

## 9. ランタイム & コンパイル

```
Lexer → Parser(糖衣解展) → AST
 ├ Type + Affine Check
 ├ Phase Check (open→freeze)
 ├ IR  (linear)          → WASM gen (WASI+threads)
 └ Diagnostics / LSP
(SSA, SIMD backend は後フェーズ)
```

---

## 10. 予約語

`record  clone  freeze  impl  
context  with  
fun  val  mut  
then  else  while  match  
async  return  true  false  Unit`

---

## 11. **完全 BNF**

```ebnf
Program      ::= (TopDecl)*                                     ;

TopDecl      ::= RecordDecl | ImplBlock | ContextDecl
               | FunDecl | BindDecl                             ;

/* ---------- Records & Prototype ---------- */
RecordDecl   ::= "record" Ident "{" FieldDecl* "}"              ;
FieldDecl    ::= Ident ":" Type                                 ;
CloneExpr    ::= Expr "clone" RecordLit                        ;  /* open T → open T */
FreezeExpr   ::= Expr "freeze"                                 ;  /* open T → closed T */
RecordLit    ::= Ident "{" FieldInit* "}"                      ;
FieldInit    ::= Ident "=" Expr                                ;

/* ---------- Impl (methods) --------------- */
ImplBlock    ::= "impl" Ident "{" FunDecl* "}"                 ;

/* ---------- Context ---------------------- */
ContextDecl  ::= "context" Ident "{" FieldDecl* "}"            ;
WithExpr     ::= "with" "("? IdentList ")"? BlockExpr          ;
IdentList    ::= Ident ("," Ident)*                            ;

/* ---------- Functions -------------------- */
FunDecl      ::= "fun" Ident "=" ParamList BlockExpr           ;
ParamList    ::= (Param)+                                      ;
Param        ::= Ident ":" Type                                ;

/* ---------- Bindings --------------------- */
BindDecl     ::= ("val" | "mut" "val") Ident "=" Expr          ;
PipeOp       ::= "|>" | "|>>" | "|"                            ;

/* ---------- Expressions ------------------ */
Expr         ::= ThenExpr                                      ;
ThenExpr     ::= WhileExpr ( "then" BlockExpr
                   ( "else" WhileExpr "then" BlockExpr )*
                   ( "else" BlockExpr )? )?                    ;

WhileExpr    ::= MatchExpr ( MatchExpr "while" BlockExpr )?    ;
MatchExpr    ::= PipeExpr ( "match" MatchBlock )?              ;
MatchBlock   ::= "{" (Pattern "=>" BlockExpr)+ "}"             ;

PipeExpr     ::= CallExpr ( PipeOp CallExpr )*                 ;

CallExpr     ::= SimpleExpr+                                   /* OSV */
               | "(" ArgList ")" SimpleExpr                    ;
ArgList      ::= Expr ("," Expr)*                              ;

SimpleExpr   ::= Literal
               | Ident
               | RecordLit
               | "(" Expr ")"
               | CloneExpr
               | FreezeExpr
               | WithExpr
               | BlockExpr                                     ;

BlockExpr    ::= "{" Stmt* Expr? "}"                           ;

Stmt         ::= BindDecl | Expr                               ;

/* ---------- Types ------------------------ */
Type         ::= Ident | Ident "<" TypeList ">"                ;
TypeList     ::= Type ("," Type)*                              ;

Literal      ::= IntLit | FloatLit | StringLit | CharLit | "true" | "false" | "Unit" ;
```

*字句規則（コメント, 文字列, 数値など）・パターンは割愛。*

---

### 12. ランタイム構成と WASM 出力

```
┌───────────────┐
│  Source .rl   │
├───────────────┤
│  Frontend     │  糖衣展開 → 型+アフィン → open/freeze チェック
├───────────────┤
│  Linear IR    │  (MVP)  ──▶  wasm32 (no-GC)       ┐
│  (SSA phase)  │  (opt) ─▶  wasm32+SIMD / WebGPU ──┘
└───────────────┘
```

* **ランタイム GC は持たない**。閉じた record・Arena 解放で管理。
* `async then` は **co-await トランスフォーマ**で CPS 化してから WASM の stack switch へ。

---

### 13. 制限のメリット・デメリット（開発者ガイド）

| 項目                | 利点                  | 注意点 & 回避策                                      |
| ----------------- | ------------------- | ---------------------------------------------- |
| アフィン変数            | 所有権が一目で分かる・GC ゼロ    | 共有したいときは `clone` コストが発生。設計段階で最小化する。            |
| 派生 = clone/freeze | デザイナが JSON で差分だけ書ける | `freeze` を忘れると open record が混入。CI で “未凍結チェック”。 |
| OSV & 後置制御        | データフローが読み線一本        | 演算子は infix 例外。複雑算式は括弧 or パイプで分割する。             |
| with コンテキスト       | 資源リークを静的排除          | ネスト過多は可読性低下 → Linter: 深さ 3 以上警告。               |

---

### 13. 現在の実装状況

#### ✅ 動作確認済み機能
- 基本的な関数定義と呼び出し
- 算術演算（+, -, *, /）
- 文字列の表示（println）
- 整数の表示（print_int）
- パッケージマネージャー（warder）
- OSV構文による関数呼び出し
- パイプ演算子（|>）
- Kotlinスタイルのセミコロンフリー構文

#### 🚧 実装中・制限のある機能
- 条件式（`then`/`else`構文の解析は可能だが実行時に制限）
- 再帰関数（基本的な解析は可能だが実行に制限）
- 複雑なアフィン型の使用（複数回参照での制限）
- パターンマッチング（一部のケースで制限）

#### ⚠️ 構文上の注意点
- 可変変数は `mut val x = value` を使用（`val mut x = value` は無効）
- 関数定義では `fun add = x:Int y:Int { x + y }` 形式を使用
- 複雑な式では括弧やセミコロンで適切に区切る

### 14. 標準ライブラリロードマップ

| カテゴリ                 | v1.0 同梱                  | 後方互換        |
| -------------------- | ------------------------ | ----------- |
| **IO / HTTP / JSON** | ✔ (WASM std + thin-HTTP) | 安定          |
| **async then**       | ✔ (CPS, thread fallback) | 安定          |
| **Arena allocator**  | ✔                        | API 不変      |
| **ECS モジュール**        | add-on (row 型)           | semver で別管理 |
| **SIMD/GPU backend** | α 版                      | SSA IR を拡張  |

---

### 15. 未来の拡張フック

| 機能                    | gate 名        | 導入指針                  |
| --------------------- | ------------- | --------------------- |
| Row 型 + コンポ自動 SoA     | `ecs_row`     | モジュールON/OFFで本体に影響させない |
| yield キーワード (ブロック値明示) | `yield_block` | Linter 警告で済むなら見送り     |
| Object Algebra モジュール  | `algebra`     | 高階型を別コンパイルステージで実装     |
