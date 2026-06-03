# Option型の仕様

> 注: この文書は実装設計ノートです。構文例は現行の言語仕様に合わせています。

## 概要
Option型は値が存在するかしないかを表現する型で、null安全性を提供します。

## 構文

### Option型の定義
```restrict
// すでに言語仕様に含まれています
Option<T>
```

### Optionの生成
```restrict
// Some: 値がある場合
val x = Some(42)      // Option<Int>
val y = Some("hello") // Option<String>

// None: 値がない場合
val z = None          // Option<T> (型は文脈から推論)
```

### パターンマッチング
```restrict
fun unwrap_or: (opt: Option<Int32>, default: Int32) -> Int32 = {
    opt match {
        Some(n) => { n }
        None => { default }
    }
}
```

### 推奨される使用方法
```restrict
// 失敗する可能性のある関数
fun safe_divide: (a: Int32, b: Int32) -> Option<Int32> = {
    b == 0 then {
        None
    } else {
        Some(a / b)
    }
}

// 使用例
fun main: () = {
    val result = (10, 0) safe_divide
    result match {
        Some(n) => { n |> print_int }
        None => { "Division by zero!" |> println }
    }
}
```

## 実装計画

1. **AST拡張**
   - `Some(expr)` コンストラクタ
   - `None` リテラル
   - パターンマッチングでのSome/Noneパターン

2. **型チェッカー**
   - Option<T>型のサポート
   - Some/Noneの型推論
   - パターンマッチングでの網羅性チェック

3. **コード生成**
   - タグ付きユニオンとして実装
   - discriminant (0=None, 1=Some) + value
