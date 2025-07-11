# リストパターンマッチング仕様

## 概要
Restrict Languageにおけるリストのパターンマッチング機能の仕様。

## サポートするパターン

### 1. 空リストパターン
```
match list {
    [] => "empty"
    _ => "not empty"
}
```

### 2. 固定長パターン
```
match list {
    [a] => "one element: " + a
    [a, b] => "two elements"
    [a, b, c] => "three elements"
    _ => "other"
}
```

### 3. Head/Tailパターン (cons パターン)
```
match list {
    [] => "empty"
    [head | tail] => "head: " + head + ", tail has " + tail.length + " elements"
}
```

### 4. 複合パターン
```
match list {
    [] => "empty"
    [x] => "singleton: " + x
    [first, second | rest] => "at least two elements"
    _ => "impossible"
}
```

## 実装方針

### AST拡張
Pattern enumに以下を追加：
- `EmptyList` - 空リスト `[]`
- `ListCons(Box<Pattern>, Box<Pattern>)` - `[head | tail]`
- `ListExact(Vec<Pattern>)` - `[a, b, c]` 固定長リスト

### パーサー
- `[]` を空リストパターンとして認識
- `[pattern1, pattern2, ...]` を固定長パターンとして認識
- `[pattern | pattern]` をcons パターンとして認識

### 型チェック
- パターンがリスト型と一致することを確認
- cons パターンの場合、headは要素型、tailはリスト型

### コード生成
1. リストの長さをチェック
2. 各要素を取り出してパターンマッチ
3. cons パターンの場合は、tail部分のサブリストを作成

## 制限事項

1. ネストしたパターンは初期実装では制限される可能性
2. 可変長の中間要素は扱わない（例：`[a, ..., z]`）
3. パフォーマンスを考慮し、tail部分は新しいリストを作成せずビューとして扱う可能性