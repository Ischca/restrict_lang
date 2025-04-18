# Restrict Language

**スコープベースの静的型付け言語**  
WASMにコンパイルされる

---

## 組み込み型

| 型             | 型表記          | 例                                 |
| -------------- | --------------- | ---------------------------------- |
| 整数型         | `Int`           | `10 let x: Int`                    |
| 浮動小数点数型 | `Float`         | `10.0 let x: Float`                |
| 真偽値型       | `Boolean`       | `false let x: Bool`                |
| 文字列型       | `String`        | `"ten" let x: String`              |
| 文字型         | `Char`          | `'c' let x: Char`                  |
| 単位型         | `Unit`          | `() let x: Unit`                   |
| オプション型   | `Option<T>`     | `Some(10) let x: Option<Int>`      |
| タプル型       | `(T1, T2, ...)` | `(10, "ten") let x: (Int, String)` |
| リスト型       | `List<T>`       | `[1, 2, 3] let x: List<Int>`       |
| 配列型         | `Array<T>`      | `[1, 2, 3] let x: Array<Int>`      |
| 関数型         | `T1 => T2`      | `n => n + 1 let x: Int => Int`     |

---

## 特徴

### 語順

Restrict言語は **OSV（Object-Subject-Verb）** 語順を採用しています。これは、**目的語**で始まり、**主語**が続き、**動詞**で終わる構造です。日本語のように主語が省略可能であり、以下のように記述します：

```ocaml
"Sam" human let sam
"Orange" let orange

orange sam.eat  // SamがOrangeを食べる
```

#### OSV語順のメリットとスコープベース・コンテキストバインドとの連携

1. **可読性と直感性の向上**
   - **オブジェクトの先行**: 最も重要なデータや対象を即座に把握できます。
   - **文脈の明確化**: 操作対象が明確になり、コードの意図を迅速に理解できます。

2. **一貫性と簡潔性**
   - **一貫した構造**: コード全体に一貫性が生まれます。
   - **スコープの合成と連結**: スコープの管理が直感的に行えます。

3. **フォーカスの強調**
   - **重要なオブジェクトの強調**: プログラマーが重要なデータに集中できます。
   - **選択肢の限定**: 必要な操作に集中でき、生産性が向上します。

### スコープベース

Restrict言語では、全ての式が関数であり、全ての関数がスコープです。スコープは第一級オブジェクトとして扱われ、柔軟なスコープ管理と再利用性を提供します。

#### スコープの第一級オブジェクト化

スコープを第一級オブジェクトとして扱うことで、スコープ自体を変数や関数のように操作できます。これにより、スコープの合成や再利用が容易になり、コードのモジュール性が向上します。

```ocaml
fun limitedScope = @LimitedScope {
    // 限定された操作のみが許可される
}

limitedScope {
    // 限定された操作を実行
}
```

---

## 制限を与える

### アフィン変数

全ての変数は**アフィン変数**であり、最大1回使用されることが保証されます。これにより、リソース管理が自動化され、バグの発生を防ぎます。

### コンテキストバインド

**コンテキストバインド**により、特定のコンテキストを持つスコープからしか呼び出すことができないスコープを作成できます。これにより、操作の安全性とコンテキストの明確化が図られます。

#### コンテキストの定義と使用

**コンテキスト**は、`context`キーワードを使用して静的に定義され、型システムに統合されています。コンテキストは、特定の制約や許可された操作を定義し、コンパイル時に検証されます。

```ocaml
context Transactional {
    Datasource let ds
    Connection let conn
}
```

#### コンテキストバインドされた関数の定義

関数やスコープに対して、どのコンテキストから呼び出せるかを明示的に指定します。`@ContextName`というアノテーションを使用します。

```ocaml
fun save = @Transactional student: Student {
    studentDaoScope { dao ->
        if student.id == null {
            student dao.insert then
        } else {
            student dao.update then
        }
    }
}
```

#### コンテキスト内での関数呼び出し

`with ContextName`を使用して、特定のコンテキストを持つスコープを作成し、その中でコンテキストバインドされた関数を呼び出します。

```ocaml
with Transactional {
    student save
}
```

#### コンテキスト外での関数呼び出しの制限

コンテキスト外からコンテキストバインドされた関数を呼び出そうとすると、コンパイル時にエラーが発生します。

```ocaml
fun main = {
    student save  // エラー：Transactionalコンテキスト外からは呼び出せない
}
```

#### コンテキストの合成と継承

コンテキスト間で合成や継承を行うことで、柔軟な設計が可能です。ただし、コンテキストの継承が必要かどうかは議論の余地があります。

```ocaml
context BaseContext {
    // 基本的な制約や定義
}

context ExtendedContext = BaseContext + {
    // BaseContextを合成した追加の制約や定義
}
```

---

### 提供の制限

特定の型に定義された関数以外の呼び出しを制限するスコープを作成できます。これにより、スコープ内で許可された操作のみが実行可能となります。

```ocaml
fun function1 = @AllowedScope {
    // 処理
}

fun function2 = @AllowedScope {
    // 処理
}

fun restrictedScope = @AllowedScope fn: () => Unit {
    function1
    function2
    fn
}

restrictedScope {
    function1
    function2
}
```

---

## 実例

### 単純な例

```ocaml
"Hello, World!" print
```

```ocaml
1 2 + print                 // 3
"Hello" uppercase print     // HELLO
```

### FizzBuzz

```ocaml
fun fizzBuzz = n: Int {
    if n > 1 then {
        n - 1 fizzBuzz
    }

    if n % 15 == 0 then "FizzBuzz"
    else if n % 5 == 0 then "Buzz"
    else if n % 3 == 0 then "Fizz"
    else n
    |> println
}

20 fizzBuzz
```

---

## スコープと関数

### スコープ

```ocaml
// 無名スコープ
{}
```

### 関数定義と呼び出し

```ocaml
fun greet = name {
    "Hello, " + name print
}

"World" greet  // Hello, World
```

### 高階関数

```ocaml
fun sandwich = fn: () => Unit {
    "start" println
    fn
    "end" println
}

{
    "Hello" println
} sandwich

// 出力
// start
// Hello
// end
```

### クロージャ

```ocaml
fun createCounter = {
    mut let counter = 0
    fun count = {
        counter = counter + 1
        counter
    }
    count
}

counter let createCounter
counter print  // 1
counter print  // 2
counter print  // 3
```

### ネスト

```ocaml
fun scopeA = {
    3 let numA
}

fun scopeB = {
    4 let numB
}

scopeA {
    numA print  // 3

    scopeB {
        numA print  // 3
        numB print  // 4
    }
}
```

### 合成

```ocaml
fun scopeA = {
    3 let numA
}

fun scopeB = {
    4 let numB
}

scopeA + scopeB {
    numA print  // 3
    numB print  // 4
}
```

### 連結

```ocaml
fun scopeA = {
    3 let numA
}

fun scopeB = num: Int {
    num print
}

scopeA
15 scopeB  // 15

scopeA {
    numA + 10
} scopeB  // 13
```

## 制御構文
Restrict言語では、制御の分岐を **スコープ規則** として表現します。代表的な構文は以下の2種類です。

1. **二分岐**：`then`/`else`
2. **多分岐**：`match`

### 1. 二分岐: `then` / `else`

Restrictでは、ブール条件 (`Boolean`) に対する二分岐を **`then`** / **`else`** のペアで記述します。  
**書式**:

```ocaml
cond then {
    // cond が true の場合に実行されるスコープ
} else {
    // cond が false の場合に実行されるスコープ
}
```

- **`cond`**：ブール値を返す式 (例: `n > 0`)  
- **`then { ... }`**：`cond` が `true` の場合に評価されるブロック(スコープ)  
- **`else { ... }`**：`cond` が `false` の場合に評価されるブロック(スコープ)

#### 多段分岐 (チェーン)

`then/else` は **「`else cond2 then { ... }`」** という形で多段分岐を連鎖できます。

```ocaml
cond1 then {
    // cond1 == true
} else cond2 then {
    // cond1 == false && cond2 == true
} else {
    // 上記2つともfalseのとき
}
```

複数の分岐が増えて可読性が下がる場合は、後述の `match` 構文を利用するのを推奨します。

### 2. 多分岐: `match`

複数のパターンを列挙し、どのパターンに一致したかで分岐する場合は **`match`** を使います。  
基本的な書式は以下のとおりです。

```ocaml
expr match {
    pattern1 => {
        // pattern1 に合致した場合
    }
    pattern2 => {
        // pattern2 に合致した場合
    }
    ...
    else => {
        // どのパターンにも合致しない場合
    }
}
```

- **オブジェクト(式) = `expr`**  
- **動詞 = `match`**  
- 波括弧 `{ ... }` の中に、複数の `pattern => { ... }` と、デフォルト処理として `else => { ... }` を記述します。

#### パターン

`pattern` は将来的に **リテラル一致**・**型パターン**・**構造パターン**などをサポートする予定です（`n == 0` のような条件式を直接書く場合には二分岐チェーンでも対応可）。

#### 使用例

```ocaml
n match {
    0 => {
        "n is zero" println
    }
    1 => {
        "n is one" println
    }
    else => {
        "n is neither 0 nor 1" println
    }
}
```

// for文
for i in 0..9 step 2 {
    i print
}

// while文
0 mut let i
while i < 10 {
    i print
    i = i + 1
}
```

---

## 内包表記

```ocaml
// [2, 4, 6, 8, 10]
[x for x in 1..10 if x % 2 == 0] let evenNumbers
```

---

## マイルストーン

- 型推論の強化（コンテキストを含む）
- 例外処理の導入
- 並列処理のサポート
- メタプログラミング機能
- 継続渡しスタイル（CPS）の実装
- コンパイラのセルフホスト化

---

## 理論的裏付け

### 認知負荷理論（Cognitive Load Theory）

- **概要**: 人間の認知負荷を軽減し、効率的な情報処理を促進します。
- **適用**: OSV語順やコンテキストバインドにより、開発者が必要な情報に集中しやすくなります。

### 情報構造（Information Structure）

- **概要**: 情報の焦点やトピックを明確化します。
- **適用**: OSV語順とコンテキストの明示により、コードの意図が明確になります。

### ソフトウェアアーキテクチャとデザインパターン

- **コマンドパターン**: 操作対象を先に示すことで、操作の一貫性と再利用性を向上させます。
- **データフローアーキテクチャ**: データの流れを直感的に理解できます。

### フロー理論（Flow Theory）

- **概要**: 開発者がフロー状態に入りやすくなります。
- **適用**: コードの自然な流れと構造を提供します。

### カテゴリ理論（Category Theory）

- **概要**: スコープとコンテキストを数学的にモデル化します。
- **適用**: スコープ管理とコンテキストの操作に一貫性と安全性をもたらします。

---

## 今後の展望

### コンテキスト機能の拡張と最適化

- **コンテキスト間の関係性の明確化**
  - コンテキストの合成や包含を通じて、柔軟な設計を可能にします。
  - コンテキストの継承が必要かどうかは、今後の議論で決定します。

- **オーバーロードとネームシャドーイングの検討**
  - コンテキストの合成時における名前の衝突やオーバーロードの扱いを明確に定義します。

---

## 開発手順

### 開発環境の設定

このプロジェクトは、[Visual Studio Code Remote - Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)を使用して、コンテナ化された環境で開発を行います。

1. [Visual Studio Code](https://code.visualstudio.com/)をインストール
2. [Remote - Containers拡張機能](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)をインストール
3. このリポジトリをクローン
4. Visual Studio Codeでリポジトリを開く
5. ウィンドウの左下にある緑色のボタンをクリックし、`Reopen in Container`を選択

### ビルド

```bash
cargo build
```

### 実行

```bash
cargo run
```

### テスト

```bash
cargo test
```
