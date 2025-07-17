# Restrict Language の非同期・並行処理設計

## 概要

このドキュメントは、Restrict Languageの独自の特徴（アフィン型、OSV構文、ゼロGC、コンテキストベースのリソース管理）を考慮した非同期・並行処理モデルを探求します。

## 言語の制約と機会

### 制約：
- **アフィン型**：値は最大1回しか使用できない
- **GCなし**：アリーナによる手動メモリ管理
- **WASMターゲット**：限定的なスレッドサポート（SharedArrayBuffer/Atomics）
- **ランタイムなし**：軽量な実行モデル

### 機会：
- **アフィン型でデータ競合を防止**：デフォルトで共有可変状態なし
- **コンテキストブロック**：非同期操作の自然なスコープ
- **OSV構文**：非同期チェーンをより読みやすくする可能性
- **明示的リソース管理**：非同期リソース処理に最適

## 提案モデル

### 1. **アフィンFuture**（Rust風、アフィン型向けに適応）

```restrict
// Futureは待機時に消費される（アフィン！）
type Future<T> = {
    poll: Self -> PollResult<T>
}

type PollResult<T> = 
    | Ready(T)
    | Pending(Future<T>)  // 次のpoll用の新しいfutureを返す

// OSV構文での使用
fun fetchUser = userId: Int32 {
    val future = (userId) http.get("/users/{id}");
    future  // Futureが返される、まだ実行されていない
}

fun main = {
    val userFuture = (123) fetchUser;
    val user = userFuture await;  // Futureがここで消費される
    user print;
}
```

### 2. **リニアチャネル**（CSPスタイル、アフィン型を活用）

```restrict
// チャネルエンドポイントはアフィン - 適切なクリーンアップを保証
record Channel<T> {
    sender: Sender<T>
    receiver: Receiver<T>
}

// チャネル作成は、アフィンな部分に分割される
fun createChannel<T> = {
    // 実装は(Sender<T>, Receiver<T>)を返す
}

// 使用例
fun worker = receiver: Receiver<Int32> {
    receiver receive match {
        Some(value) => {
            value process;
            receiver worker;  // receiverでの再帰呼び出し
        }
        None => { unit }  // チャネルが閉じられた
    }
}

fun main = {
    val (sender, receiver) = createChannel();
    
    // ワーカーを起動（receiverを消費）
    (receiver) spawn(worker);
    
    // 値を送信（senderはアフィン、線形に使用される必要がある）
    (42) sender.send;
    (84) sender.send;
    sender.close;  // senderが消費される
}
```

### 3. **エフェクトハンドラー**（Restrict向けの新しいアプローチ）

```restrict
// エフェクトはコンテキストとして宣言される
context Async {
    await: Future<T> -> T
    spawn: (fn() -> T) -> Future<T>
    parallel: List<Future<T>> -> Future<List<T>>
}

// ハンドラーは実装を提供する
handler AsyncHandler for Async {
    // 実装詳細
}

// 使用は既存のコンテキストシステムと組み合わせる
fun fetchData = {
    with Async {
        val user = ("/users/123") http.get |> await;
        val posts = ("/posts?user=123") http.get |> await;
        (user, posts)
    }
}
```

### 4. **セッション型**（プロトコル安全性のためのアフィン型活用）

```restrict
// セッション型はプロトコルの準拠を保証
type ClientSession = 
    | SendRequest(Request) -> AwaitResponse
    | Close

type AwaitResponse = 
    | ReceiveResponse(Response) -> ClientSession

// 使用はプロトコルが守られることを保証
fun httpClient = session: ClientSession {
    session match {
        SendRequest(cont) => {
            val request = Request { method: "GET", path: "/" };
            val awaitSession = (request) cont;
            awaitSession handleResponse
        }
        Close => { unit }
    }
}
```

### 5. **コルーチンコンテキスト**（Kotlin風、Restrict適応）

```restrict
// コルーチンコンテキストは実行を管理
context Coroutine {
    suspend: fn() -> Unit
    resume: Unit -> Unit
    yield: T -> Unit
}

// コンテキストネスティングによる構造化並行性
fun processItems = items: List<Item> {
    with Coroutine {
        items |> forEach(|item| {
            with Coroutine {  // 子コルーチン
                item process;
                yield;  // 協調的スケジューリング
            }
        })
    }
}
```

## 推奨アプローチ：ハイブリッドモデル

最良の側面を組み合わせる：

### 1. **コア：アフィンFuture + リニアチャネル**
- 単一の非同期値にはFuture
- ストリームと通信にはチャネル
- 両方とも安全性のためにアフィン型を活用

### 2. **高レベル：エフェクトハンドラー**
- 構造化並行性のためにコンテキストシステムを使用
- `with`ブロックでクリーンな構文を提供
- 異なる非同期戦略を可能にする

### 3. **OSV最適化構文**

```restrict
// OSVでの順次非同期
fun fetchUserWithPosts = userId: Int32 {
    with Async {
        val user = ("/users/{userId}") http.get |> await;
        val posts = ("/posts?user={userId}") http.get |> await;
        UserWithPosts { user: user, posts: posts }
    }
}

// OSVでの並列非同期
fun fetchParallel = urls: List<String> {
    with Async {
        urls 
        |> map(|url| (url) http.get)  // Futureを作成
        |> parallel                    // すべてを待つ
        |> await
    }
}
```

## 新しいアイデア

### 1. **時間的アフィン型**
```restrict
// 値は時間的境界を持つ
temporal<'t> record Connection {
    socket: Socket
}

fun handleRequest = conn: Connection<'t> {
    with lifetime<'t> {
        // 接続はこのスコープ内でのみ有効
        conn.read |> process |> conn.write
    }  // 接続は自動的に閉じられる
}
```

### 2. **データフロー変数**（Oz風）
```restrict
// 単一代入非同期変数
type Flow<T> = {
    bind: T -> Unit      // 一度だけ呼び出し可能（アフィン）
    wait: Unit -> T      // バインドまでブロック
}

fun dataflow = {
    val (flow, binder) = createFlow();
    
    spawn(|| {
        val result = expensiveComputation();
        (result) binder.bind;  // binderが消費される
    });
    
    // バインド前に複数回読み取り可能
    val value = flow.wait();
}
```

このアプローチの**最大の革新**は、**アフィン型システムがデータ競合を言語レベルで防ぐ**ことです。これにより、従来のlockやmutexが不要になり、より安全で理解しやすい並行プログラミングが可能になります。

どの部分に最も興味がありますか？まずは基本的なFutureとawaitから実装してみるのはいかがでしょうか？