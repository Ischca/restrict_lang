# レコードとプロトタイプ

レコードは、Restrict Languageで関連するデータをまとめるための基本的な型です。v0.0.1の公開例では、フィールド定義、レコードリテラル、パターンのすべてで現在の`:`構文を使います。

## レコード定義

レコード定義では、各フィールドに型注釈を書きます。

```restrict
record Point {
    x: Int32
    y: Int32
}

record Person {
    name: String
    age: Int32
    email: String
}
```

## レコードの作成

レコードリテラルでは、フィールド名と値を`:`で結びます。複数フィールドは`,`で区切ります。

```restrict
val origin = Point { x: 0, y: 0 }
val point = Point { x: 10, y: 20 }

val alice = Person {
    name: "Alice",
    age: 30,
    email: "alice@example.com"
}
```

## フィールドアクセス

フィールドアクセスはドット記法です。

```restrict
fun x_coord: (point: Point) -> Int32 = {
    point.x
}
```

複数のフィールドを使う場合は、値を一度だけ受け取り、その式の中で必要なフィールドを読みます。

```restrict
fun sum_point: (point: Point) -> Int32 = {
    point.x + point.y
}
```

## レコードを返す関数

メソッド構文ではなく、通常の関数とOSV呼び出しを使います。

```restrict
fun translate: (point: Point, dx: Int32, dy: Int32) -> Point = {
    Point { x: point.x + dx, y: point.y + dy }
}

fun main: () -> Point = {
    val point = Point { x: 3, y: 4 }
    (point, 1, 2) translate
}
```

## レコードのパターンマッチング

レコードは`match`式で分解できます。

```restrict
fun describe_point: (point: Point) -> String = {
    point match {
        Point { x: 0, y: 0 } => { "原点" }
        Point { x: 0, y } => { "y軸上" }
        Point { x, y: 0 } => { "x軸上" }
        Point { x, y } => { "その他" }
    }
}
```

フィールド名と同じ名前で束縛する場合は短縮できます。

```restrict
fun area_hint: (point: Point) -> Int32 = {
    point match {
        Point { x, y } => { x * y }
    }
}
```

別名で束縛する場合も`:`を使います。

```restrict
fun add_renamed: (point: Point) -> Int32 = {
    point match {
        Point { x: px, y: py } => { px + py }
    }
}
```

## cloneとfreeze

プロトタイプ操作は`clone`と`freeze`で表します。フィールドを差し替える場合は、対象値の後に`.clone { ... }`を書きます。

```restrict
fun move_x: (point: Point, next_x: Int32) -> Point = {
    point.clone { x: next_x }
}

fun freeze_origin: () -> Point = {
    val origin = Point { x: 0, y: 0 }
    origin freeze
}
```

継承風の大きなプロトタイプ階層やメソッド解決は、v0.0.1の公開ガイドでは前提にしません。現在の例では、レコード値を通常の関数に渡して処理します。

## ネストしたレコード

レコードは他のレコードをフィールドとして持てます。

```restrict
record Address {
    street: String
    city: String
    country: String
}

record Employee {
    name: String
    age: Int32
    address: Address
}

fun employee_city: (employee: Employee) -> String = {
    employee.address.city
}

fun main: () -> String = {
    val employee = Employee {
        name: "Bob",
        age: 25,
        address: Address {
            street: "123 Main St",
            city: "Springfield",
            country: "USA"
        }
    }

    employee |> employee_city
}
```

## 重要な注意事項

- フィールド定義とフィールド初期化は`:`を使います。
- レコードリテラルの複数フィールドは`,`で区切ります。
- レコード値もアフィン型の規則に従います。
- レコードを処理する公開例では、メソッド風の呼び出しではなくOSV形式を使います。
- パターンマッチングでは、必要に応じてワイルドカード`_`やスプレッド`..._`で残りのケースを扱います。

## v0.0.1の範囲外

次の項目は将来の設計対象です。

- メソッド定義用の専用構文
- デフォルトフィールド値
- フィールド単位の可視性修飾子
- 派生実装
