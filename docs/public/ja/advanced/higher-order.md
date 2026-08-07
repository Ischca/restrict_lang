# 高階関数とコレクション変換

高階関数は、関数値を受け取ったり返したりする関数です。Restrictでも呼び出しは
OSV語順のままで、通常の値と関数値を呼び出す関数より前に置きます。

コレクション変換では、もう一つRestrictらしい読み方ができます。高階の動詞が
最後の関数引数をレキシカルスコープとして開くため、関数名を先頭へ移動せずに
コールバック本体を動詞の後ろへ置けます。

## 関数値は通常の引数

名前付き関数やラムダは、通常の引数として渡せます。

```restrict
fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun apply_twice: (value: Int32, transform: Int32 -> Int32) -> Int32 = {
    value |> transform |> transform
}

fun main: () -> Int32 = {
    (5, double) apply_twice
}
```

インラインラムダも、同じグループ化されたOSV呼び出しを使います。

```restrict
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    (numbers, |value| value * 2) map
}
```

コンテナ、コールバック、それ以前の引数はすべて呼び出しの目的語なので、
`map`、`filter`、`fold`より前に置かれます。

## 現在のコレクション操作

v0.0.1コンパイラは、次の高階操作をpreludeへ登録しています。

| 操作 | 現在の入力 | コールバック | 結果 |
| --- | --- | --- | --- |
| `map` | `List<T>` | `T -> U` | `List<U>` |
| `map` | `Option<T>` | `T -> U` | `Option<U>` |
| `filter` | `List<T>` | `T -> Boolean` | `List<T>` |
| `filter` | `Option<T>` | `T -> Boolean` | `Option<T>` |
| `fold` | `List<T>`と初期値`U` | `(U, T) -> U` | `U` |

`map`はリストの各要素、または`Some`のペイロードを変換します。`None`は
`None`のままです。`filter`は述語がtrueになるリスト要素を残します。
`Option`では、条件を満たす`Some`を残し、それ以外を`None`にします。`fold`は
リストを左から右へ走査し、累積値と現在の要素をreducerへ渡します。

`Array`と`Result`は、現在のコンパイラのコンテナ動作には含まれません。
`fold`は`List`専用です。内部の`Container.Item`と`Container.Mapped<U>`は
ソースから見えるformではなく、ユーザー定義型からadoptすることもまだできません。

## スコープ動詞節

最後に残った仮引数が関数型なら、動詞はその関数本体を後続スコープとして
開けます。

```restrict
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3]
    numbers map {
        it * 2
    }
}
```

ヘッダーのないブロックには、文脈的な束縛`it`が1つ導入されます。コンパイラは
コンテナ要素型を期待される引数型として使い、通常ラムダを通して節を展開します。

```restrict
numbers map { it * 2 }
// 同じコールバックモデル:
(numbers, |it| { it * 2 }) map
```

この形式はコレクションだけにハードコードされた特別構文ではありません。
最後に残った引数が関数である任意の呼び出しでスコープを開けます。

```restrict
fun apply: (value: Int32, transform: Int32 -> Int32) -> Int32 = {
    value |> transform
}

fun main: () -> Int32 = {
    41 apply {
        it + 1
    }
}
```

完成した`41 apply { ... }`節は、ほかのOSV呼び出しと同じように値を生成します。

## 暗黙バインダーと明示バインダー

短い単項コールバックには、暗黙`it`形式を使えます。

```restrict
values filter {
    it > 0
}
```

本体が長い場合や、名前自体に意味がある場合は引数を明示します。

```restrict
values map { |value|
    val shifted = value + 1;
    shifted * 2
}
```

複数引数のコールバックには明示バインダーが必要です。`fold`のreducerは、
累積値を先、現在の要素を後に受け取ります。

```restrict
(values, 0) fold { |total, value|
    total + value
}
```

引数なしのスコープには明示的な空ヘッダー`{ || ... }`を使います。これは最後の
引数が`() -> T`である高階関数に使えますが、`map`、`filter`、`fold`には
当てはまりません。

## 完成した節をつなぐ

スコープ動詞節は左から右へ結合します。完成した各節が次の動詞の目的語になります。

```restrict
fun main: () -> Int32 = {
    val values = [1, 2, 3]
    val selected = values map {
        it + 1
    } filter {
        it > 2
    }
    (selected, 0) fold { |total, value|
        total + value
    }
}
```

このプログラムは次の順に読めます。

1. `[1, 2, 3]`を`[2, 3, 4]`へmapする
2. 完成したmap結果を`[3, 4]`へfilterする
3. その結果を`7`へfoldする

後続のパイプも、完成した節全体の結果を受け取ります。

```restrict
values map { it + 1 } |> list_count
```

## 型推論

高階呼び出しは、両方向に型情報を与えます。

- コンテナ型がコールバックの要素型を決める
- `map`本体が変換後の要素型を決める
- 宣言された結果型からコールバック結果を制約できる
- `filter`の結果には`Boolean`が必要
- 初期値とreducer結果が`fold`の累積値型を決める
- `identity`のような名前付きジェネリック関数は、期待されるコールバック型から
  特殊化できる

```restrict
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3]
    (numbers, identity) map
}
```

空コレクションや値のないOptionでは、アノテーションや別の具体的な利用が必要に
なる場合があります。Restrictはフォールバック型を選ばず、未解決の型を報告します。

## Optionの変換

現在の`Option`コンテナ動作にも、同じスコープ形式を使えます。

```restrict
fun increment_if_present: (value: Option<Int32>) -> Option<Int32> = {
    value map {
        it + 1
    }
}

fun keep_positive: (value: Option<Int32>) -> Option<Int32> = {
    value filter {
        it > 0
    }
}
```

これはコンテナとしてのmap/filterであり、独立した`option_map` APIではありません。
v0.0.1の`fold`は引き続き`List`専用です。

## アフィン値とキャプチャ

スコープ付きコールバックも、通常ラムダと呼び出しと同じアフィン規則に従います。

- 非Copyのコレクション束縛を`map`、`filter`、`fold`へ渡すと、その呼び出しで
  束縛を使用する
- コールバック引数とキャプチャした束縛は、通常のCopyまたはアフィン動作を保つ
- 波括弧によってキャプチャしたアフィン値の利用回数が増えることはない
- コールバック本体は、受け取る関数の契約に従って0回、1回、または複数回実行される

暗黙スコープを2重にすると、どちらのfocusも`it`になるためRestrictは拒否します。
少なくとも片方のバインダーを明示してください。

```restrict
groups map { |group|
    group map {
        it + 1
    }
}
```

外側を`group`と明示することで、値の流れとキャプチャ意図が明確になります。

## 形式の選び方

コールバックにすでに適切な名前がある場合や、関数値として選択した場合は、
通常の関数引数を使います。

```restrict
(numbers, normalize) map
```

コールバックが1つの変換にだけ属する場合は、スコープ動詞節を使います。

```restrict
numbers map {
    it + 1
}
```

複数引数、ネストしたスコープ、長い本体には明示バインダーを使います。

```restrict
(numbers, 0) fold { |total, number|
    total + number
}
```

3形式はいずれも同じ高階関数モデルです。違うのはコールバックの導入方法と、
ローカルな値の流れをどの程度明確に読めるかだけです。

## 関連項目

- [関数](../guide/functions.md) - 関数宣言、関数値、関数型
- [OSV語順](../guide/osv-order.md) - 節単位のOSV合成
- [型システム](../guide/types.md) - 関数型とジェネリックコンテナ
- [標準ライブラリ](../reference/stdlib.md) - 現在のprelude surface
