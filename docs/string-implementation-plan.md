# Restrict Language 文字列実装計画

## 概要
WASM MVPの制約の中で、実用的な文字列サポートを実装する。

## 実装フェーズ

### Phase 1: 基本的な文字列サポート（MVP）
1. **文字列定数プール**
   - 全ての文字列リテラルをデータセクションに格納
   - コンパイル時に文字列定数を収集
   - 各文字列に一意のIDを割り当て

2. **メモリレイアウト**
   ```
   [ptr: i32] -> [length: i32][UTF-8 bytes...]
   ```

3. **基本操作**
   - 文字列リテラルの生成
   - 文字列の表示（デバッグ用）
   - 文字列の比較

### Phase 2: 文字列操作（拡張）
1. **文字列連結**
   - 新しいメモリ領域を確保
   - 既存の文字列をコピー

2. **部分文字列**
   - スライス操作の実装

3. **文字列補間**
   - テンプレート文字列のサポート

### Phase 3: 高度な機能
1. **GCまたはArenaアロケータ**
   - メモリ管理の自動化

2. **StringRef提案への対応準備**
   - 将来的なWASM標準への移行パス

## 実装例

### 文字列リテラルのコード生成
```wasm
(module
  ;; データセクション
  (data (i32.const 1024) "Hello, World!")
  
  ;; 文字列を返す関数
  (func $get_hello (result i32 i32)
    i32.const 1024  ;; pointer
    i32.const 13    ;; length
  )
)
```

### Restrict Languageでの使用例
```restrict
fun main = {
    val message = "Hello, World!";
    message println
}
```

## 技術的考慮事項

1. **エンコーディング**: UTF-8を採用（Rustとの互換性）
2. **メモリ管理**: 初期実装では静的確保、後にArenaアロケータ
3. **JavaScript連携**: TextEncoder/TextDecoderを使用
4. **パフォーマンス**: 文字列境界の通過を最小限に

## 将来の展望

WebAssembly StringRef提案が実装されれば、ネイティブな文字列型が利用可能になる。
その際は、既存のAPIを維持しつつ内部実装を切り替える。