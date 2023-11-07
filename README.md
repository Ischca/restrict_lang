# 言語

スコープベースの静的型付け言語

## 組み込み型

| 型             | Types    | Example                                      |
| -------------- | -------- | -------------------------------------------- |
| 整数型         | Int      | `val x: Int = 10`                            |
| 浮動小数点数型 | Float    | `val x: Float = 10.0`                        |
| 真偽値型       | Boolean  | `val x: Bool = false`                        |
| 文字列型       | String   | `val x: String = "ten"`                      |
| 文字型         | Char     | `val x: Char = 'c'`                          |
| Unit           | Unit     | `val x: Unit = ()`                           |
| Option         | Option   | `val x: Option(int) = Some(10)`              |
| タプル型       | Tuple    | `val x: (int, string) = (10, "ten")`         |
| リスト型       | List     | `val x: List<Int> = [1, 2, 3]`               |
| 配列型         | Array    | `val x: Array<Int> = [1, 2, 3]`              |
| 関数型         | Function | `val x: (int, int) => int = (a, b) => a + b` |

## 特徴

### 語順

OSV 型（Object - Subject - Verb）の語順を採用している。  
`Orange Sam eat.`のように、目的語で始まり、主語が続き、動詞で終わる。  
また、日本語のように主語が抜けてもよい。

```ocaml
val sam = "Sam" human
val orange = "Orange"

orange sam.eat  // Sam eat Orange


fun human = name: String {
    val ate: String[] = []
    fun eat = food: String {
        food
        |> ate.push  // パイプによりfoodの結果をpush関数に渡す
        |> s -> `$name eat $s` print  // ラムダ式も可能
    }
}
```

### スコープベース

関数型言語のように、この言語は全ての式が関数であり、全ての関数がスコープである。
全てのスコープは第一級オブジェクトである。

### 制限を与える

#### アフィン変数

全ての変数はアフィン変数であり、最大 1 回使用されることが保証される。

#### コンテキストバインド

特定のコンテキストを持つスコープからしか呼び出すことが出来ないスコープを作成できる。

```ocaml
ctx Transactional = <ds: Datasource, conn: Connection>
fun with_transactional = fn: () => (Int, String) {
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
    val student = "Takeshi" "221569" Student
    student save
    |> print  // true
}

```

#### 提供の制限

特定の型に定義された関数以外の呼び出しを制限するスコープを作成できる。

```ocaml
@AllowedInRestrictedScope
fun function1 = {
    // 処理
}

@AllowedInRestrictedScope
fun function2 = {
    // 処理
}

@AllowedInRestrictedScope
fun function3 = {
    // 処理
}

fun restrictedFun using @AllowedInRestrictedScope, function3 = {
    // function1, function2, function3 が使用可能な制限付きスコープ
    ...
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

// 変数
val foo = "Foo"

// 空の関数
fun f = {}

// 引数があるスコープ
fun one = arg1 {
    arg1 print
}
// 引数が2つ以上の場合
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
    // ここでの`c` の中身はtwoの戻り値なので`unit`
    "Ischca" one
} // Hello I'm Ischca
```

#### 高階関数

引数にスコープを受け取ることで、中間に処理を挟むことができる

```ocaml
fun sandwich = fn: () => Unit {
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
  fun count = n:Int {
    n + 1 count
  }
  0 count
}

var counter = createCounter
counter print // 0
counter print // 1
counter print // 2
```

#### ネスト

```ocaml
fun scopeA = {
    val numA = 3
}

fun scopeB = {
    val numB = 4
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
    val numA = 3
}

fun scopeB = {
    val numB = 4
}

scopeA + scopeB {
    numA print // 3
    numB print // 4
}
```

#### 連結

```ocaml
fun scopeA = {
    val numA = 3
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
val n = 1
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

`処理 繰り返し条件(or 配列) 呼出し条件`で記述する

```ocaml
{{it} [1..10] {true}}
```

`繰り返し条件`以外は省略可能  
以下の場合、各要素が加工されず、条件に従って偶数のみのリストが返る

```ocaml
{[1..10] {it % 2 == 0}}
// [2, 4, 6, 8, 10]
```

以下の場合、各数値が 2 乗されたリストが返る

```ocaml
{{it * it} [1..10]}
// [1, 4, 9, 16, 25, 36, 49, 64, 81, 100]
```

`繰り返し条件`は配列である必要は無い  
以下の例では、RPC への接続が成功するまで無限に繰り返している

```ocaml
{{it - 1 + it - 2} {sleep 1000 | fix {getRpc | isOk}} {it > 1}}
{
    {it - 1 + it - 2}
    {sleep 1000 | fix {getRpc | isOk}}
    {it > 1}
}

```

ここで、fix 関数は以下の定義とする

```ocaml
fun fix = <T> f: () -> T { f fix g }
```

```ocaml
// [[1, 2], [3, 4], [5, 6]]
// -> [1, 3. 5]
// -> [2, 4, 6]

val l = {{it} [1..6] {}} // [[1, 2], [3, 4], [5, 6]]
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
