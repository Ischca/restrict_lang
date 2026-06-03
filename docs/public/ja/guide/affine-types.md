# アフィン型

アフィン型は、Restrict Language のメモリ安全性とリソース管理の基盤です。値は原則として**最大 1 回**だけ使用できます。

## アフィン型とは？

型理論において、アフィン型は以下の中間に位置します。

- 線形型: ちょうど 1 回使用されなければならない
- 無制限型: 何回でも使用できる

Restrict Language では、値は 0 回または 1 回使用できますが、それ以上は通常できません。複数回使う必要がある束縛には `mut val` を使います。

## 基本ルール

### ルール1: 単一使用

値が一度使用されると、同じ束縛をもう一度使用することはできません。

```restrict
val message = "once"
val moved = message

// エラー: message はすでに使用されています。
// val again = message
```

### ルール2: 関数は引数を消費する

関数に値を渡すと、その束縛は消費されます。

```restrict
fun consume: (value: String) -> () = {
    value |> println
}

val text = "hello"
text |> consume

// エラー: text はすでに使用されています。
// text |> consume
```

### ルール3: 可変束縛は複数回使える

`mut val` は複数回の使用や再代入が必要な場合に使います。

```restrict
mut val counter = 0

counter = counter + 1
counter = counter + 2

val final_count = counter
```

構文は `mut val` です。

## なぜアフィン型？

### 1. GCなしのメモリ安全性

アフィン型は、値が不用意に共有され続けることを防ぎます。

```restrict
record Token {
    value: String
}

fun redeem: (token: Token) -> String = {
    token.value
}

val token = Token { value: "secret" }
val value = token |> redeem

// エラー: token は redeem で消費されています。
// token |> redeem
```

### 2. 移動後使用の防止

```restrict
val list = [3, 1, 2]
val sorted = list |> sort

// エラー: list は sort で消費されています。
// val count = list |> list_count

val sorted_count = sorted |> list_count
```

### 3. 明確なデータフロー

```restrict
val saved = "raw data"
    |> validate
    |> transform
    |> save
```

各ステップは前の値を受け取り、新しい値を返します。

## 複数回使いたい場合

### `mut val` を使う

単純に同じ束縛を複数回読む必要がある場合は、可変束縛にします。

```restrict
mut val label = "ready"

label |> println
label |> println
```

### 新しい値として返す

関数が値を完全に消費しない設計なら、次に使う値を戻り値として返します。

```restrict
fun increment: (count: Int32) -> Int32 = {
    count + 1
}

val start = 0
val next = start |> increment
val final_count = next |> increment
```

### clone と freeze を使う

プロトタイプベースのレコードでは、`clone` と `freeze` で派生値を作れます。

```restrict
record Settings {
    retries: Int32
    timeout: Int32
}

val base = Settings { retries: 3, timeout: 10 } freeze
val strict = base.clone { timeout: 3 }
```

## リソース表現

一度だけ使われるべき値を型で表すと、二重使用を防げます。

```restrict
record Ticket {
    id: Int32
}

fun use_ticket: (ticket: Ticket) -> Int32 = {
    ticket.id
}

val ticket = Ticket { id: 100 }
val used_id = ticket |> use_ticket

// エラー: ticket はすでに消費されています。
// ticket |> use_ticket
```

## 状態マシン

アフィン型は状態遷移の表現にも使えます。

```restrict
record ConnectionClosed {
}

record ConnectionOpen {
    handle: Int32
}

fun connect: (state: ConnectionClosed) -> ConnectionOpen = {
    ConnectionOpen { handle: 1 }
}

fun send: (conn: ConnectionOpen, data: String) -> ConnectionOpen = {
    conn
}

fun close: (conn: ConnectionOpen) -> ConnectionClosed = {
    ConnectionClosed { }
}

val closed0 = ConnectionClosed { }
val open1 = closed0 |> connect
val open2 = (open1, "Hello") send
val closed1 = open2 |> close
```

`open2` を `close` に渡した後は、同じ接続値で送信することはできません。

## v0.0.1 の current example ではないもの

次の説明は将来の設計対象、または実装状況の確認が必要な領域です。

- 一般的な借用構文
- レコードに対する非消費メソッド
- ファイルやソケットの標準ライブラリ API
- temporal affine types による自動 cleanup

## ベストプラクティス

1. 単一使用値で自然に動作する API を設計する
2. 複数回使う必要がある場合だけ `mut val` を使う
3. 消費しない設計にしたい値は、次に使う値として返す
4. プロトタイプ派生には `clone` と `freeze` を使う
5. 型エラーを、データフロー設計を見直す手がかりにする

## 関連項目

- [構文リファレンス](syntax.md) - v0.0.1 の基本構文
- [関数](functions.md) - OSV 呼び出しと関数定義
