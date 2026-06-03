# パターンマッチング

パターンマッチングは、値の形に応じて分岐しながら中身を取り出すための機能です。Restrict Languageでは、`value match { ... }`というOSVに沿った形で書きます。

## 基本構文

```restrict
value match {
    pattern => { result }
    _ => { default_value }
}
```

各アームの本体は`{ }`で囲みます。`match`式全体が値を返すため、各アームの結果型はそろえる必要があります。

## リテラルパターン

数値、文字列、真偽値などのリテラルに一致できます。

```restrict
fun describe_number: (number: Int32) -> String = {
    number match {
        0 => { "ゼロ" }
        1 => { "一" }
        42 => { "答え" }
        _ => { "その他" }
    }
}
```

## 変数束縛

パターンに識別子を書くと、マッチした値をその名前に束縛します。

```restrict
fun double: (value: Int32) -> Int32 = {
    value match {
        number => { number * 2 }
    }
}
```

## Optionパターン

`Option<T>`は`Some(value)`と`None`で分岐します。

```restrict
fun value_or_zero: (maybe: Option<Int32>) -> Int32 = {
    maybe match {
        Some(value) => { value }
        None => { 0 }
    }
}
```

## Resultパターン

`Result<T, E>`は`Ok(value)`と`Err(error)`で分岐します。

```restrict
fun result_or_zero: (result: Result<Int32, String>) -> Int32 = {
    result match {
        Ok(value) => { value }
        Err(message) => { 0 }
    }
}
```

## リストパターン

リストは空、正確な長さ、先頭と残りに分ける形でマッチできます。

```restrict
fun first_or_zero: (numbers: List<Int32>) -> Int32 = {
    numbers match {
        [] => { 0 }
        [head | tail] => { head }
    }
}
```

```restrict
fun sum_pair: (numbers: List<Int32>) -> Int32 = {
    numbers match {
        [left, right] => { left + right }
        _ => { 0 }
    }
}
```

## レコードパターン

レコードパターンでは、フィールドを取り出したり、特定の値に一致させたりできます。

```restrict
record Point {
    x: Int32
    y: Int32
}

fun describe_point: (point: Point) -> String = {
    point match {
        Point { x: 0, y: 0 } => { "原点" }
        Point { x: 0, y } => { "y軸上" }
        Point { x, y: 0 } => { "x軸上" }
        Point { x, y } => { "その他" }
    }
}
```

短縮構文では、フィールド名と同じ名前で束縛します。

```restrict
fun multiply_point: (point: Point) -> Int32 = {
    point match {
        Point { x, y } => { x * y }
    }
}
```

別名で束縛する場合は`:`を使います。

```restrict
fun add_point: (point: Point) -> Int32 = {
    point match {
        Point { x: px, y: py } => { px + py }
    }
}
```

## スプレッドパターン

レコードの一部だけを取り出し、残りを無視する場合は`..._`を使えます。

```restrict
record User {
    name: String
    role: String
    department: String
}

fun describe_user: (user: User) -> String = {
    user match {
        User { role: "admin", name, ..._ } => { "管理者: " + name }
        User { department: "support", name, ..._ } => { "サポート: " + name }
        User { name, ..._ } => { "ユーザー: " + name }
    }
}
```

残りのフィールドを値として束縛する`...rest`もレコードパターンの最後に置きます。

```restrict
fun split_profile: (user: User) -> String = {
    user match {
        User { name, ...profile } => { name }
    }
}
```

## ネストしたパターン

レコードとリストのパターンは組み合わせられます。

```restrict
record Person {
    name: String
    age: Int32
}

record Company {
    name: String
    employees: List<Person>
}

fun first_employee_name: (company: Company) -> String = {
    company match {
        Company { employees: [], ..._ } => { "従業員なし" }
        Company { employees: [first | rest], ..._ } => { first.name }
    }
}
```

## 網羅性

`match`式はすべての可能性を扱う必要があります。すべてのケースを列挙するか、`_`で残りを受けます。

```restrict
fun safe_value: (value: Option<Int32>) -> Int32 = {
    value match {
        Some(number) => { number }
        None => { 0 }
    }
}
```

## 重要な注意事項

- `match`は`value match { ... }`の形で書きます。
- 各アームの本体は`{ }`で囲みます。
- フィールドパターンとフィールド初期化は`:`を使います。
- アフィン型システムにより、束縛した値も最大1回の使用に従います。
- パターンガードやタプルパターンの詳細は、v0.0.1の公開範囲外です。
