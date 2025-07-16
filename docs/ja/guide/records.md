# レコードとプロトタイプ

レコードは、Restrict Languageで関連するデータをグループ化する主な方法です。継承と不変性のために`clone`と`freeze`操作を使用したプロトタイプベースのシステムを使用しています。

## レコード定義

レコードはスペース区切りのフィールドで定義されます：

```rust
record Point {
    x: Int
    y: Int
}

record Person {
    name: String
    age: Int
    email: String
}
```

## レコードの作成

スペース区切りのフィールド値でレコードインスタンスを作成：

```rust
val origin = Point { x: 0 y: 0 }
val point = Point { x: 10 y: 20 }

val alice = Person {
    name: "Alice"
    age: 30
    email: "alice@example.com"
}
```

## フィールドアクセス

ドット記法を使用してレコードフィールドにアクセス：

```rust
val p = Point { x: 5 y: 10 }
val x_coord = p.x  // 5
val y_coord = p.y  // 10
```

## レコードのパターンマッチング

レコードはパターンマッチングで分解できます：

```rust
val point = Point { x: 10 y: 20 }

// 特定の値にマッチ
point match {
    Point { x: 0 y: 0 } => { "原点" }
    Point { x: 0 y } => { "y軸上" }
    Point { x y: 0 } => { "x軸上" }
    Point { x y } => { x + y }
}

// 短縮フィールド束縛
point match {
    Point { x y } => { x * y }  // xとyはフィールド値に束縛される
}

// 束縛の名前変更
point match {
    Point { x: px y: py } => { px + py }
}
```

## メソッド

`impl`ブロックを使用してレコードのメソッドを実装：

```rust
impl Point {
    fun distance = self: Point {
        // 原点からの距離を計算
        val x_sq = self.x * self.x
        val y_sq = self.y * self.y
        sqrt(x_sq + y_sq)
    }
    
    fun translate = self: Point dx: Int dy: Int {
        Point { x: self.x + dx y: self.y + dy }
    }
}

// 使用方法
val p = Point { x: 3 y: 4 }
val dist = p distance        // 5.0
val moved = (p, 1, 2) translate  // Point { x: 4 y: 6 }
```

## プロトタイプシステム

レコードは`clone`と`freeze`を通じてプロトタイプベースの継承をサポート：

### Clone

レコードの可変コピーを作成：

```rust
val base = Point { x: 0 y: 0 }
val mut copy = base clone

// クローンを変更
copy.x = 10
// baseは変更されない
```

### Freeze

レコードを不変にする：

```rust
val mut point = Point { x: 5 y: 10 }
point.x = 15  // OK、pointは可変

val frozen = point freeze
// frozen.x = 20  // エラー：凍結されたレコードは変更できない
```

## ネストしたレコード

レコードは他のレコードを含むことができます：

```rust
record Address {
    street: String
    city: String
    country: String
}

record Employee {
    name: String
    age: Int
    address: Address
}

val emp = Employee {
    name: "Bob"
    age: 25
    address: Address {
        street: "123 Main St"
        city: "Springfield"
        country: "USA"
    }
}

// ネストしたフィールドへのアクセス
val city = emp.address.city
```

## 型注釈

レコードフィールドには常に型を指定：

```rust
record Config {
    host: String      // 必須の型注釈
    port: Int         // 必須の型注釈
    debug: Bool       // 必須の型注釈
}
```

## 重要な注意事項

- フィールドはカンマ区切りではなくスペース区切り
- レコード構築時のフィールド順序は重要
- レコードはアフィン型に従う - 各レコード値は最大1回しか使用できない
- レコードのパターンマッチングには網羅的なパターンまたはワイルドカードが必要

## 将来の機能

- 型パラメータを持つジェネリックレコード
- デフォルトフィールド値
- フィールドの可視性修飾子
- 派生実装（等価性など）