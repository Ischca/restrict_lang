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

## 実装の詳細設計

### 1. **Future型の内部実装**

```restrict
// 内部表現
type FutureState<T> = 
    | Pending(Waker)
    | Ready(T)
    | Consumed  // アフィン型で一度だけ使用を保証

record Future<T> {
    state: Ref<FutureState<T>>  // 内部可変性が必要
    
    // pollはselfを消費し、新しいFutureまたは結果を返す
    poll: Self -> PollResult<T>
}

// Wakerは非同期ランタイムへのコールバック
record Waker {
    wake: Unit -> Unit
}
```

### 2. **非同期ランタイムの最小実装**

```restrict
// シングルスレッドイベントループ
context EventLoop {
    // タスクキュー
    tasks: Queue<Task>
    
    // I/O完了待ちのタスク
    pending_io: Map<Handle, Waker>
    
    // タイマー
    timers: PriorityQueue<(Time, Waker)>
}

record Task {
    future: Future<Unit>
    waker: Waker
}

// ランタイムのメインループ
fun runEventLoop = {
    with EventLoop {
        loop {
            // 準備完了タスクを実行
            tasks.popFront match {
                Some(task) => {
                    task.future.poll match {
                        Ready(_) => { /* タスク完了 */ }
                        Pending(newFuture) => {
                            // 新しいfutureで再キュー
                            tasks.pushBack(Task { 
                                future: newFuture, 
                                waker: task.waker 
                            })
                        }
                    }
                }
                None => {
                    // I/Oまたはタイマーを待つ
                    waitForEvents()
                }
            }
        }
    }
}
```

### 3. **async/await構文糖の変換**

```restrict
// ソース（提案構文）
async fun fetchUserData = userId: Int32 {
    val user = await http.get("/users/{userId}");
    val profile = await http.get("/profiles/{userId}");
    UserData { user: user, profile: profile }
}

// 変換後（脱糖）
fun fetchUserData = userId: Int32 -> Future<UserData> {
    // ステートマシンとして実装
    enum State {
        Start
        AwaitingUser(Future<User>)
        AwaitingProfile(User, Future<Profile>)
        Done
    }
    
    record StateMachine {
        state: State
        userId: Int32
    }
    
    Future {
        poll: |self| {
            self.state match {
                Start => {
                    val userFuture = http.get("/users/{self.userId}");
                    Pending(Future { 
                        state: AwaitingUser(userFuture),
                        userId: self.userId 
                    })
                }
                AwaitingUser(userFuture) => {
                    userFuture.poll match {
                        Ready(user) => {
                            val profileFuture = http.get("/profiles/{self.userId}");
                            Pending(Future {
                                state: AwaitingProfile(user, profileFuture),
                                userId: self.userId
                            })
                        }
                        Pending(newUserFuture) => {
                            Pending(Future {
                                state: AwaitingUser(newUserFuture),
                                userId: self.userId
                            })
                        }
                    }
                }
                AwaitingProfile(user, profileFuture) => {
                    profileFuture.poll match {
                        Ready(profile) => {
                            Ready(UserData { user: user, profile: profile })
                        }
                        Pending(newProfileFuture) => {
                            Pending(Future {
                                state: AwaitingProfile(user, newProfileFuture),
                                userId: self.userId
                            })
                        }
                    }
                }
            }
        }
    }
}
```

### 4. **構造化並行性とキャンセレーション**

```restrict
// スコープベースのタスク管理
context TaskScope {
    children: List<TaskHandle>
    cancelled: Ref<Bool>
    
    // 新しいタスクを生成
    spawn: Future<T> -> TaskHandle<T>
    
    // すべての子タスクを待つ
    joinAll: Unit -> Unit
    
    // スコープとすべての子をキャンセル
    cancel: Unit -> Unit
}

// 使用例
fun processItems = items: List<Item> {
    with TaskScope {
        val handles = items |> map(|item| {
            spawn(async {
                item |> validate |> await;
                item |> process |> await;
                item |> save |> await;
            })
        });
        
        // すべて完了を待つか、エラーでキャンセル
        try {
            joinAll();
        } catch {
            Error(e) => {
                cancel();  // すべての子タスクをキャンセル
                throw e;
            }
        }
    }
}
```

### 5. **アフィン型による安全なストリーム処理**

```restrict
// ストリームは一度だけ消費可能
type Stream<T> = {
    next: Self -> StreamResult<T>
}

type StreamResult<T> = 
    | Item(T, Stream<T>)  // 値と継続
    | End                 // ストリーム終了

// ストリームコンビネータ
fun map<T, U> = stream: Stream<T>, f: T -> U -> Stream<U> {
    Stream {
        next: |self| {
            stream.next match {
                Item(value, nextStream) => {
                    Item(f(value), map(nextStream, f))
                }
                End => End
            }
        }
    }
}

// 非同期ストリーム処理
async fun processStream = stream: Stream<Data> {
    stream.next match {
        Item(data, rest) => {
            await processData(data);
            await processStream(rest);  // 末尾再帰
        }
        End => unit
    }
}
```

### 6. **WASM統合の考慮事項**

```restrict
// WASM用の非同期I/Oインターフェース
extern {
    // ホスト関数
    wasm_async_read: (Handle, Buffer, Size) -> Future<Size>
    wasm_async_write: (Handle, Buffer, Size) -> Future<Size>
    wasm_set_timer: (Milliseconds, Waker) -> TimerHandle
}

// プラットフォーム抽象化
trait AsyncIO {
    read: (Handle, Buffer, Size) -> Future<Size>
    write: (Handle, Buffer, Size) -> Future<Size>
}

// WASM実装
handler WasmAsyncIO for AsyncIO {
    read = |handle, buffer, size| {
        wasm_async_read(handle, buffer, size)
    }
    
    write = |handle, buffer, size| {
        wasm_async_write(handle, buffer, size)
    }
}
```

## パフォーマンス最適化

### 1. **ゼロコスト抽象化**

```restrict
// コンパイル時に最適化可能な非同期チェーン
fun optimizedChain = {
    // これは中間Futureを作らずに単一のステートマシンにコンパイルされる
    async {
        val a = await computeA();
        val b = await computeB(a);
        val c = await computeC(b);
        c
    }
}
```

### 2. **アリーナアロケータとの統合**

```restrict
// 非同期タスクごとのアリーナ
context TaskArena {
    arena: Arena
    
    // タスク完了時にアリーナ全体を解放
    runTask: Future<T> -> T
}

fun efficientAsyncProcessing = {
    with TaskArena {
        async {
            // このタスク内のすべてのアロケーションは同じアリーナを使用
            val data = await loadData();
            val processed = data |> transform |> filter |> collect;
            await saveResults(processed);
        } |> runTask
    }  // アリーナが自動的に解放される
}
```

## 次のステップ

1. **プロトタイプ実装**
   - 基本的なFuture型とpollメカニズム
   - 簡単なイベントループ
   - async/await構文の脱糖

2. **言語統合**
   - パーサーへのasync/await構文追加
   - 型チェッカーへのFuture型サポート追加
   - コード生成での非同期変換

3. **ランタイムライブラリ**
   - 基本的な非同期I/O操作
   - タイマーとスケジューリング
   - エラーハンドリングとキャンセレーション

この設計により、Restrict Languageは**メモリ安全**かつ**データ競合フリー**な非同期プログラミングを、**ゼロコスト抽象化**で実現できます。