# Restrict Language

スコープベースの静的型付け言語  
WASMにコンパイルされる

## 組み込み型

| 型             | Types    | Example                                      |
| -------------- | -------- | -------------------------------------------- |
| 整数型         | Int      | `let x: Int = 10`                            |
| 浮動小数点数型 | Float    | `let x: Float = 10.0`                        |
| 真偽値型       | Boolean  | `let x: Bool = false`                        |
| 文字列型       | String   | `let x: String = "ten"`                      |
| 文字型         | Char     | `let x: Char = 'c'`                          |
| Unit           | Unit     | `let x: Unit = ()`                           |
| Option         | Option   | `let x: Option(Int) = Some(10)`              |
| タプル型       | Tuple    | `let x: (Int, String) = (10, "ten")`         |
| リスト型       | List     | `let x: List<Int> = [1, 2, 3]`               |
| 配列型         | Array    | `let x: Array<Int> = [1, 2, 3]`              |
| 関数型         | Function | `let x: (Int, Int) => Int = (a, b) => a + b` |

## 特徴

### 語順

Restrict言語は **OSV（Object-Subject-Verb）** 語順を採用しています。これは、目的語で始まり、主語が続き、動詞で終わる構造です。日本語のように主語が省略可能であり、以下のように記述します：

```ocaml
let sam = "Sam" human
let orange = "Orange"

orange sam.eat  // Sam eat Orange
```

#### OSV語順のメリットとスコープベース・コンテキストバインドとの連携

1. **可読性と直感性の向上**
   - **オブジェクトの先行**: コードを読む際に最も重要なデータや対象を即座に把握できます。これにより、コードの意図や目的を迅速に理解しやすくなります。
   - **文脈の明確化**: オブジェクトが先に来ることで、その後に続く操作がそのオブジェクトに対して行われることが明確になります。
   - **スコープの明確化**: OSV語順は、操作対象とそのスコープが明確に関連付けられるため、スコープ管理が直感的に行えます。例えば、特定のオブジェクトに関連するスコープを先に定義することで、その後の操作がそのスコープ内で行われることが一目で分かります。

2. **一貫性と簡潔性**
   - **一貫した構造**: OSV語順により、コード全体に一貫性が生まれ、特定のパターンや構造を予測しやすくなります。
   - **簡潔な表現**: 特にチェーン操作やパイプライン処理において、オブジェクトを先に記述することでコードがシンプルになります。
   - **スコープの合成と連結**: OSV語順はスコープの合成や連結を自然に行えるようにし、複数のスコープを組み合わせる際のコードが読みやすくなります。

3. **フォーカスの強調**
   - **重要なオブジェクトの強調**: オブジェクトを先に配置することで、プログラマーは重要なデータに集中しやすくなります。
   - **選択肢の限定**: 操作対象を明確にすることで、必要な操作に集中でき、生産性が向上します。スコープやコンテキストバインドと組み合わせることで、プログラマーが意図しない操作を防ぎ、安全性を高めます。

4. **操作の優先順位の明確化**
   - **優先順位の明確化**: 操作対象とその操作の関係性が一目で分かり、複雑な操作やネストされた関数呼び出しにおいても、操作の優先順位が明確になります。
   - **コンテキストバインドとの相乗効果**: 操作対象を先に定義することで、その後に続く操作が特定のコンテキスト内で行われることが保証されます。これにより、スコープやコンテキストの整合性が保たれ、安全な操作が可能となります。

#### 理論的裏付け

1. **認知負荷理論（Cognitive Load Theory）**
   - **概要**: 人間の短期記憶には限界があり、情報の過負荷を避けるために認知負荷を最小限に抑えることが重要です。
   - **適用**: OSV語順は、操作対象を即座に把握できるため、認知負荷を軽減し、効率的な情報処理を促進します。スコープやコンテキストバインドと組み合わせることで、プログラマーが必要な情報に集中しやすくなります。

2. **情報構造（Information Structure）**
   - **概要**: 言語学では、情報の焦点やトピックを効果的に伝えるために情報構造が重要視されます。
   - **適用**: OSV語順では、操作対象（オブジェクト）が最初に来ることで、情報の焦点が明確になり、重要な情報が強調されます。スコープやコンテキストバインドと連携することで、コードの意図や責任範囲が一目で理解できます。

3. **ソフトウェアアーキテクチャとデザインパターン**
   - **コマンドパターン（Command Pattern）**: 操作をオブジェクトとして表現することで、操作の管理や拡張が容易になります。OSV語順は、操作対象を先に示すことでコマンドパターンとの親和性を高め、操作の一貫性と再利用性を向上させます。
   - **データフローアーキテクチャ（Data Flow Architecture）**: データの流れを明確にすることで、データ駆動型のアプリケーション開発が効率化されます。OSV語順は、データ（オブジェクト）が先に来ることで、データの流れと操作の流れが直感的に理解できます。

4. **フロー理論（Flow Theory）**
   - **概要**: チクセントミハイのフロー理論は、集中力とスキルが高まると「フロー状態」に入り、最適なパフォーマンスが発揮されることを示します。
   - **適用**: OSV語順は、操作対象とその操作を直感的に結びつけることで、プログラマーが自然な流れでコードを記述・理解できるようにし、フロー状態に入りやすくします。スコープベースの管理とコンテキストバインドは、コードの構造を明確にし、集中力を高めます。

### スコープベース

関数型言語のように、この言語は全ての式が関数であり、全ての関数がスコープです。全てのスコープは第一級オブジェクトであり、柔軟なスコープ管理と再利用性を提供します。

#### スコープの第一級オブジェクト化

スコープを第一級オブジェクトとして扱うことで、スコープ自体を変数や関数のように操作できます。これにより、スコープの合成や再利用が容易になり、コードのモジュール性が向上します。

```ocaml
fun withLimitedScope = @LimitedScope {
    // 限定された操作のみが許可される
}

withLimitedScope {
    // 限定された操作を実行
}
```

### 制限を与える

#### アフィン変数

全ての変数はアフィン変数であり、最大1回使用されることが保証されます。これにより、リソース管理が自動化され、バグの発生を防ぎます。

#### コンテキストバインド

特定のコンテキストを持つスコープからしか呼び出すことができないスコープを作成できます。これにより、操作の安全性とコンテキストの明確化が図られます。

```ocaml
context Transactional {
    let ds: Datasource
    let conn: Connection
}

fun with_transactional = fn: () => Result {
    conn.begin
    with Transactional fn
    |> match {
        case Ok conn.commit
        case Err conn.rollback
    }
    conn.close
}

fun studentDaoScope = @Transactional {
    StudentDao conn
}

fun save = @Transactional student: Student {
    studentDaoScope { dao ->
        if student.id == null {
            then student dao.insert
            else student dao.update
        }
    }
}

with_transactional {
    let student = "Takeshi", "221569" Student.init
    student save
    |> print  // true
}
```

#### 提供の制限

特定の型に定義された関数以外の呼び出しを制限するスコープを作成できます。これにより、スコープ内で許可された操作のみが実行可能となります。

```ocaml
fun function1 = @AllowedInRestrictedScope {
    // 処理
}

fun function2 = @AllowedInRestrictedScope {
    // 処理
}

fun function3 = @AllowedInRestrictedScope {
    // 処理
}

fun restrictedFun = @AllowedInRestrictedScope fn: () => Unit {
    // function1, function2, function3 が使用可能な制限付きスコープ
    function1
    function2
    function3
    fn
}

// restrictedFunの呼び出し例
restrictedFun {
    function1
    function2
    function3
}
```

#### 単純な例

```ocaml
"Hello, World!" print
```

```ocaml
1 + 2 print                 // 3
"Hello" uppercase print     // HELLO
```

#### FizzBuzz

```ocaml
fun fizzBuzz = n: Int {
    if n > 1 then n-1 fizzBuzz

    if n%15 == 0 then "FizzBuzz"
    else if n%5 == 0 then "Buzz"
    else if n%3 == 0 then "Fizz"
    else n
    |> println
}

20 fizzBuzz
```

#### スコープ

```ocaml
// 無名スコープ
// 代入していない場合は即時実行される
// トップレベルの記述とほぼ同じだが、外側の変数を変更できないなどスコープのルールが適用される
{}

let foo = "Foo"

fun f = {}

fun one = arg1 {
    arg1 print
}

fun two = arg1, arg2 {
    arg1 print
    arg2 print
}

// 引数がある関数の呼び出し
"Hello, ", "World!" two    // Hello, World!

// 全てのスコープは戻り値を内包したスコープを返す
// 戻り値は`context`キーワードに含まれる
// 呼び出し後にスコープを定義した場合、続けざまに実行される
"Hello ", "I'm " two { c ->
    // ここでの`c` の中身はtwoの戻り値なので`Unit`
    "Ischca" one
} // Hello I'm Ischca
```

#### 高階関数

引数にスコープを受け取ることで、中間に処理を挟むことができます。

```ocaml
fun sandwich = fn: () -> Unit {
    "start" println
    fn
    "end" println
}

"Hello" println sandwich

---

start
Hello
end
```

#### クロージャ

```ocaml
fun createCounter = {
  mut let counter = 0
  fun count = {
      counter = counter + 1
      counter
  }
  count
}

let counter = createCounter
counter print // 1
counter print // 2
counter print // 3
```

#### ネスト

```ocaml
fun scopeA = {
    let numA = 3
}

fun scopeB = {
    let numB = 4
}

scopeA {
    numA print // 3

    scopeB {
        numA print // 3
        numB print // 4
    }
}
```

#### 合成

```ocaml
fun scopeA = {
    let numA = 3
}

fun scopeB = {
    let numB = 4
}

scopeA + scopeB {
    numA print // 3
    numB print // 4
}
```

#### 連結

```ocaml
fun scopeA = {
    let numA = 3
}

fun scopeB = num:Int {
    num print
}

scopeA
|> 15 scopeB    // 15

// 引数を省略した場合、`context`が渡される
scopeA {
    numA + 10
}
|> scopeB    // 13
```

#### 制御構文

```ocaml
// if式
let n = 1
if n == 0 {
    then {
        "n is 0"
    }
    else {
        "n is not 0"
    }
}   // n is not 0

// match式
match {
    case n == 0 {
        "n is 0"
    }
    case n == 1 {
        "n is 1"
    }
    else {
        "n is not 0 and not 1"
    }
}   // "n is 1"

// for文
for (i in 0..9 step 2) {
  print(i)
} // 0123456789

// while文
while (i < 10) {
    i print
    i = i + 1
} // 0123456789
```

#### 内包表記

```ocaml
for (x <- (1 to 10) if x %2 ==0) yield x    // Scala
[x for x in range(1, 11) if x%2 == 0]       # Python
[x | x <- [1..10], x `mod` 2 == 0]          -- Haskell
// [2, 4, 6, 8, 10]
```

`処理 繰り返し条件(or 配列) 呼出し条件`で記述します。

```ocaml
{{it} [1..10] {true}}
```

`繰り返し条件`以外は省略可能です。以下の場合、各要素が加工されず、条件に従って偶数のみのリストが返ります。

```ocaml
{[1..10] {it % 2 == 0}}
// [2, 4, 6, 8, 10]
```

以下の場合、各数値が2乗されたリストが返ります。

```ocaml
{{it * it} [1..10]}
// [1, 4, 9, 16, 25, 36, 49, 64, 81, 100]
```

`繰り返し条件`は配列である必要はありません。以下の例では、RPCへの接続が成功するまで無限に繰り返しています。

```ocaml
{{it - 1 + it - 2} {sleep 1000 | fix {getRpc | isOk}} {it > 1}}
{
    {it - 1 + it - 2}
    {sleep 1000 | fix {getRpc | isOk}}
    {it > 1}
}
```

ここで、fix関数は以下の定義とします。

```ocaml
fun fix = <T> f: () -> T { f fix g }
```

```ocaml
// [[1, 2], [3, 4], [5, 6]]
// -> [1, 3, 5]
// -> [2, 4, 6]

let l = {{it} [1..6] {}} // [[1, 2], [3, 4], [5, 6]]
{{it} {l | }}
```

## マイルストーン

- 型推論
- 例外処理
- 並列処理
- 型
- メタプログラミング
- 継続
- セルフホスト
- 

## 理論的裏付け

Restrict言語の設計には、以下のような理論的な裏付けが存在します：

1. **認知負荷理論（Cognitive Load Theory）**
   - **概要**: 人間の短期記憶には限界があり、情報の過負荷を避けるために認知負荷を最小限に抑えることが重要です。
   - **適用**: OSV語順は、操作対象を即座に把握できるため、認知負荷を軽減し、効率的な情報処理を促進します。スコープベースの管理とコンテキストバインドと組み合わせることで、プログラマーが必要な情報に集中しやすくなります。

2. **情報構造（Information Structure）**
   - **概要**: 言語学では、情報の焦点やトピックを効果的に伝えるために情報構造が重要視されます。
   - **適用**: OSV語順では、操作対象（オブジェクト）が最初に来ることで、情報の焦点が明確になり、重要な情報が強調されます。スコープやコンテキストバインドと連携することで、コードの意図や責任範囲が一目で理解できます。

3. **ソフトウェアアーキテクチャとデザインパターン**
   - **コマンドパターン（Command Pattern）**: 操作をオブジェクトとして表現することで、操作の管理や拡張が容易になります。OSV語順は、操作対象を先に示すことでコマンドパターンとの親和性を高め、操作の一貫性と再利用性を向上させます。
   - **データフローアーキテクチャ（Data Flow Architecture）**: データの流れを明確にすることで、データ駆動型のアプリケーション開発が効率化されます。OSV語順は、データ（オブジェクト）が先に来ることで、データの流れと操作の流れが直感的に理解できます。

4. **フロー理論（Flow Theory）**
   - **概要**: チクセントミハイのフロー理論は、集中力とスキルが高まると「フロー状態」に入り、最適なパフォーマンスが発揮されることを示します。
   - **適用**: OSV語順は、操作対象とその操作を直感的に結びつけることで、プログラマーが自然な流れでコードを記述・理解できるようにし、フロー状態に入りやすくします。スコープベースの管理とコンテキストバインドは、コードの構造を明確にし、集中力を高めます。

5. **カテゴリ理論（Category Theory）**
   - **概要**: カテゴリ理論は、数学とコンピュータサイエンスにおける抽象的な構造を研究する分野であり、プログラミング言語の構造や関数の合成を形式的に捉えるために有用です。
   - **適用**: スコープをカテゴリ理論でモデル化することで、スコープ間の関係性やコンテキストバインドの操作を形式的に定義できます。これにより、スコープ管理が数学的に一貫性を持ち、安全性が保証されます。

これらの理論的基盤により、Restrict言語はユーザーの心理的ニーズに応じた設計となっており、安全性と生産性の向上を実現します。

## Development

### Use Dev Container

This project uses [Visual Studio Code Remote - Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers) to develop in a containerized environment.

1. Install [Visual Studio Code](https://code.visualstudio.com/)
2. Install [Visual Studio Code Remote - Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
3. Clone this repository
4. Open this repository in Visual Studio Code
5. Click the green button in the lower left corner of the window and select `Reopen in Container`

### Build

```bash
$ cargo build
```

### Run

```bash
$ cargo run
```

### Test

```bash
$ cargo test
```
