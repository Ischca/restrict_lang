# Module System Roadmap

**Created**: 2025-01-11
**Status**: Implementation Phase
**Target**: v1.0 Release

---

## Design Decisions

| 項目 | 決定 |
|------|------|
| 可視性 | 明示的export (private by default) |
| Prelude | 暗黙的インポート |
| 修飾付きインポート | なし (常に直接名前を使う) |

---

## Phase 1: Prelude自動インポート ✅ COMPLETED

**Goal**: ユーザーが何もimportしなくても基本関数が使える

**Status**: Completed on 2025-01-11

### 1.1 Preludeファイル作成 ✅
- [x] 現在のパーサーで動く構文でstd/prelude.rlを書き直す
- [x] 最小限の関数セット定義 (16関数)
  - [x] not (Boolean)
  - [x] identity_int, identity_bool
  - [x] eq_int, ne_int, lt_int, le_int, gt_int, ge_int (比較)
  - [x] add, sub, mul, div, mod, neg (算術)
  - [x] unit, panic, assert (ユーティリティ)

### 1.2 組み込み関数の整理 ✅
- [x] 型チェッカーの組み込み関数を整理 (register_std_prelude)
- [x] print/println は polymorphic 実装済み (register_std_io)
- [x] 組み込み vs Prelude定義の境界を明確化

### 1.3 Prelude自動読み込み ✅
- [x] TypeChecker::new()でPreludeを自動ロード (register_builtins → register_std_prelude)
- [x] Preludeの関数を初期スコープに登録
- [x] テスト: Prelude関数が使えることを確認

### 1.4 Codegen対応 ✅
- [x] Prelude関数のWASM生成 (generate_prelude_functions)
- [x] 組み込み関数との連携

**Note**: `and`, `or`, `xor`, `abs`, `max`, `min` はmatch armでのaffine制約により保留。
今後のaffine checker改善で対応予定。

---

## Phase 2: Import解決 ✅ COMPLETED

**Goal**: `import module.{name}` でモジュールから関数を取り込める

**Status**: Completed on 2025-01-11

### 2.1 ModuleResolver統合 ✅
- [x] main.rsにModuleResolver統合
- [x] 検索パス設定 (ソースファイルディレクトリ, std/)
- [x] モジュールファイル探索ロジック

### 2.2 Export収集 ✅
- [x] パース時にexport宣言を収集
- [x] エクスポートテーブル構築
- [x] private関数のフィルタリング (exportされたもののみ公開)

### 2.3 Import処理 ✅
- [x] import文のパース (既存のパーサー使用)
- [x] モジュールパス → ファイルパス解決
- [x] インポートされた名前をスコープに追加
- [ ] 名前衝突の検出とエラー (TODO: 将来の改善)

### 2.4 型チェッカー統合 ✅
- [x] インポートされた関数の型情報取得 (register_imported_decl)
- [x] インポートされたRecord型の登録
- [x] インポートされたContext型の登録

### 2.5 循環依存検出 ✅
- [x] resolving set による依存追跡
- [x] 循環検出アルゴリズム
- [x] 明確なエラーメッセージ ("Import chain involves: ...")

### 2.6 Codegen統合 ✅
- [x] インポートした関数のWASM生成
- [x] インライン展開 (単一WASMファイル出力)

**Example:**
```rl
// std/test_module.rl
export fun double: (x: Int) -> Int = { x * 2 }

// main.rl
import test_module.{double}
fun main: () -> Int = { 5 double }  // → 10
```

---

## Phase 3: Codegen統合

**Goal**: 複数モジュールから単一WASMを生成

### 3.1 モジュール収集
- [ ] 使用されるモジュールの収集
- [ ] 依存順序でのソート
- [ ] 未使用モジュールの除外 (dead code elimination)

### 3.2 名前マングリング
- [ ] モジュール間での名前衝突回避
- [ ] 内部関数名の生成規則
- [ ] エクスポート名の保持

### 3.3 コード結合
- [ ] 全モジュールのWASM関数を結合
- [ ] グローバル変数の統合
- [ ] メモリレイアウトの調整

### 3.4 最適化
- [ ] 未使用関数の削除
- [ ] インライン展開 (小さな関数)
- [ ] 定数畳み込み

---

## Phase 4: 標準ライブラリ整備

**Goal**: 実用的な標準ライブラリを提供

### 4.1 std/io
- [ ] print, println (polymorphic)
- [ ] read_line (WASI)
- [ ] file operations (WASI)

### 4.2 std/list
- [ ] map, filter, fold
- [ ] head, tail, length
- [ ] concat, reverse

### 4.3 std/option
- [ ] map, flatMap
- [ ] unwrap_or, expect
- [ ] is_some, is_none

### 4.4 std/string
- [ ] length, concat
- [ ] split, join
- [ ] substring, contains

### 4.5 std/math
- [ ] abs, min, max
- [ ] pow, sqrt
- [ ] trigonometric (if needed)

---

## Phase 5: パッケージマネージャ (Warder)

**Goal**: サードパーティライブラリの配布と利用

### 5.1 warder.toml設計
- [ ] パッケージメタデータ形式
- [ ] 依存関係記述
- [ ] バージョン指定

### 5.2 ローカルビルド
- [ ] warder build コマンド
- [ ] warder run コマンド
- [ ] warder test コマンド

### 5.3 パッケージ公開 (将来)
- [ ] レジストリ設計
- [ ] warder publish
- [ ] warder install

---

## Success Metrics

### Phase 1 完了条件
- [ ] `42 print` がPreludeインポートなしで動作
- [ ] テストが全て通過

### Phase 2 完了条件
- [ ] `import math.{abs}` で関数をインポート可能
- [ ] 循環依存でエラー
- [ ] 未エクスポート関数へのアクセスでエラー

### Phase 3 完了条件
- [ ] 複数ファイルプロジェクトがコンパイル可能
- [ ] 生成されるWASMが正しく動作

### Phase 4 完了条件
- [ ] 基本的なプログラムが標準ライブラリで書ける
- [ ] ドキュメント完備

### Phase 5 完了条件
- [ ] サードパーティライブラリを作成・利用可能
- [ ] 依存解決が正しく動作

---

## 現在のフォーカス

**Phase 1: Prelude自動インポート** から開始

理由:
1. 最もシンプルで効果が高い
2. 他のPhaseの基盤となる
3. ユーザー体験を即座に改善

---

## Notes

- 各Phaseは独立してテスト可能にする
- 後方互換性を維持する
- エラーメッセージは常に明確に
