# Form と静的ポリモーフィズム

Form は、Restrict における明示的な振る舞いの契約です。OSV 呼び出し、
アフィン所有権、WebAssembly の直接呼び出しを保ったまま、ジェネリック関数が
必要とする振る舞いを名前で表せます。

v0.0.1 slice は意図的に小さくしています。

- `form` は非ジェネリックで、必須メソッドのシグネチャだけを持つ
- 各メソッドは完全に型付けされ、先頭は `self: Self`
- 具体的な非ジェネリック record が `takes` で form を採用する
- 型パラメータは `<T of A + B>` で必要な form を列挙する
- コンパイラは vtable や実行時辞書を使わず、呼び出しを単相化する

## Form の宣言と採用

```restrict
pub form Labelled {
    fun label: (self: Self) -> String
}

record Badge {
    text: String
}

Badge takes Labelled {
    fun label: (self: Badge) -> String = {
        self.text
    }
}

fun read_label: <T of Labelled>(value: T) -> String = {
    value |> label
}
```

適合は明示的です。同名の通常 `impl` メソッドがあっても、対応する `takes`
宣言がなければ form を満たしません。`takes` は form が要求するメソッドを
過不足なく実装し、`Self` を対象 record 型へ置き換えた完全なシグネチャを
記述します。適合判定は位置と型で行われるため、パラメータ名は契約と異なっても
構いません。

`pub form` は別の Restrict ソースモジュールから import できます。
`takes` 自体には `pub` を付けず、form と nominal record を公開します。

## 複数の制約

`+` は、具体型が列挙されたすべての form を満たすことを意味します。

```text
fun inspect: <T of Display + Labelled>(value: T) -> String = { ... }
```

この初期 slice では、同じ具体型の通常 `impl` と form adoption は一つの
selector 名前空間を共有するため、二つの宣言から同じ selector を公開できません。
複数の generic bound が同じ selector を公開する場合も曖昧性エラーになり、
暗黙の優先順位は付きません。

## アフィンな receiver

Form メソッドも通常のアフィン関数です。非 Copy record を `self` として渡すと
値を消費します。Form の解決が値を借用、複製、再評価することはありません。
Form の採用によって型が Copy になることもありません。

## Display と出力

コンパイラは `display: (self: Self) -> String` を要求する `Display` form と、
`String`、`Int32`、`Int64`、`Float64`、`Boolean`、`Char`、`()` の adoption を
提供します。ユーザー record は明示的に採用します。

```restrict
record Notice {
    text: String
}

Notice takes Display {
    fun display: (self: Notice) -> String = {
        self.text
    }
}

fun main: () -> () = {
    42 |> print
    " · " |> print
    Notice { text: "records too" } |> println
}
```

`print` と `println` は任意の Display 値を受け取ります。`eprint` と
`eprintln` は String 専用のままで、`print_int` と `print_float` も互換用に
残ります。`display`、`print`、`println` はコンパイラ予約の直接呼び出し先で、
トップレベル関数や通常/custom form の method selector として宣言できません。
組み込み自体を first-class 関数値として捕捉することもまだできません。例外は
`RecordName takes Display` 内の `display` メソッドだけです。

## 現在の境界

関連型、generic form、generic/conditional `takes`、default method、enum adoption、
negative bound、adoption の選択的 import、existential form value、dynamic dispatch
は今後の機能です。コレクション組み込みが内部で使う Container projection は、
`Container` やソース関連型の公開を意味しません。
