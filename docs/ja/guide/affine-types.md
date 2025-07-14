# アフィン型

アフィン型は、Restrict Languageのメモリ安全性とリソース管理の基盤です。アフィン型システムでは、すべての値は**最大1回**しか使用できません。この制約は最初は制限的に見えるかもしれませんが、強力な保証を提供し、クリーンで効率的なコードを促進します。

## アフィン型とは？

型理論において、アフィン型は以下の中間に位置します：
- **線形型**：ちょうど1回使用されなければならない
- **無制限型**：何回でも使用できる

Restrict Languageはアフィン型を使用し、値は**0回または1回**使用できますが、それ以上は使用できません。

## 基本ルール

### ルール1：単一使用

値が一度使用されると、再び使用することはできません：

```restrict
val x = 42
val y = x    // xはyに移動される
// val z = x // エラー: xはすでに使用されている！
```

### ルール2：関数は引数を消費する

関数に値を渡すと、関数はそれを消費します：

```restrict
fun consume = x:Int { x + 1 }

val num = 10
val result = num consume
// num consume  // エラー: numはすでに使用されている！
```

### ルール3：可変変数は複数回使用可能

`mut`キーワードは複数回の使用を許可します：

```restrict
val mut counter = 0
counter = counter + 1  // 最初の使用
counter = counter + 2  // 2回目の使用 - OK！
val final = counter    // ここで消費される
```

## なぜアフィン型？

### 1. GCなしのメモリ安全性

アフィン型は以下を保証します：
- 値が誤ってエイリアスされない
- リソースが決定的にクリーンアップされる
- ガベージコレクタが不要

```restrict
val file = open_file("data.txt")
val contents = file read_all  // fileは消費される
// file.close()  // エラー: fileはすでに使用されている！
// 心配無用 - fileは自動的にクローズされる
```

### 2. 一般的なバグの防止

多くのバグは、無効化された後に値を使用することから生じます：

```restrict
val list = [1, 2, 3]
val sorted = list sort      // listは消費される
// val first = list[0]      // エラー: 移動後の使用を防ぐ
val first = sorted[0]       // OK: ソート済みバージョンを使用
```

### 3. 明確なデータフロー

アフィン型はデータフローを明示的にします：

```restrict
val data = fetch_data()
|> validate
|> transform
|> save  // 各ステップが前の値を消費
```

## アフィン型との作業

### 必要に応じたクローン

値を複数回使用する必要がある場合は、明示的にクローンします：

```restrict
val original = ComplexData { /* ... */ }
val copy = original.clone()

process1(original)  // originalを消費
process2(copy)      // コピーを使用
```

### 借用パターン

消費せずに読み取り専用アクセスをする場合は、アクセサ関数を使用：

```restrict
record Person {
    name: String,
    age: Int,
}

impl Person {
    // アクセサはselfを消費しない
    fun get_name = self:Person -> String {
        self.name.clone()  // コピーを返す
    }
}

val person = Person { name: "Alice", age: 30 }
val name = person.get_name()  // personを消費しない
val age = person.age          // これはpersonを消費する
```

### 戻り値

関数は値を「返す」ことができます：

```restrict
fun process_and_return = data:Data -> Data {
    // データを処理...
    data  // 消費する代わりに返す
}

val data = create_data()
val processed = data process_and_return
// processedはさらなる使用のために利用可能
```

## 高度なパターン

### アフィンリソース

一度だけ使用されるべきリソースの管理に最適：

```restrict
record Token {
    value: String,
}

fun use_token = token:Token {
    // トークンは使用後に消費される
    authenticate(token.value)
}

val token = Token { value: "secret" }
token use_token
// token use_token  // エラー: 二重使用を防ぐ！
```

### 状態マシン

アフィン型は状態遷移をエンコードできます：

```restrict
record ConnectionClosed { }
record ConnectionOpen { handle: Int }

fun connect = _:ConnectionClosed -> ConnectionOpen {
    ConnectionOpen { handle: establish_connection() }
}

fun send = conn:ConnectionOpen, data:String -> ConnectionOpen {
    // データを送信...
    conn  // 再利用のために接続を返す
}

fun close = conn:ConnectionOpen -> ConnectionClosed {
    close_handle(conn.handle)
    ConnectionClosed { }
}

// 使用は正しい状態遷移を強制する
val conn = ConnectionClosed { } connect
val conn = (conn, "Hello") send
val closed = conn close
// クローズ後は送信できない！
```

## ベストプラクティス

1. **制約を受け入れる** - 単一使用値で自然に動作するAPIを設計
2. **`mut`は控えめに使用** - 本当に複数回の使用が必要な場合のみ
3. **明示的にクローン** - コピーを意図的で可視化する
4. **消費しないものは返す** - 関数は完全に処理しない値を返すべき
5. **コンパイラに従う** - 型エラーはしばしば設計の改善を示す

## よくある誤解

### 「制限的すぎる」

アフィン型は表現できることを制限しません。リソース使用について明示的であることを要求するだけです。実際のプログラムのほとんどの値は自然に一度だけ使用されます。

### 「すべてをクローンする必要がある」

実際には、クローンはまれです。良いAPI設計と可変束縛の適切な使用により、クローンの必要性のほとんどがなくなります。

### 「Rustの所有権のよう」

類似していますが、Restrict Languageのアフィン型はより単純です：
- 借用やライフタイムなし
- 移動とコピーの区別なし
- 可変変数は複数回使用可能

## 関連項目

- [変数と可変性](variables.md) - アフィン型が変数使用に与える影響
- [メモリ管理](../advanced/memory.md) - アリーナ割り当てとアフィン型
- [リソース管理](../patterns/resources.md) - リソース管理のパターン