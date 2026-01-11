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

## Phase 1: Prelude自動インポート

**Goal**: ユーザーが何もimportしなくても基本関数が使える

### 1.1 Preludeファイル作成
- [ ] 現在のパーサーで動く構文でstd/prelude.rlを書き直す
- [ ] 最小限の関数セット定義
  - [ ] print<T: Display> (polymorphic)
  - [ ] println<T: Display> (polymorphic)
  - [ ] identity<T>
  - [ ] not, and, or (Boolean)

### 1.2 組み込み関数の整理
- [ ] 型チェッカーの組み込み関数を整理 (register_std_*)
- [ ] Preludeからre-exportする形に統一
- [ ] 組み込み vs Prelude定義の境界を明確化

### 1.3 Prelude自動読み込み
- [ ] TypeChecker::new()でPreludeを自動ロード
- [ ] Preludeの関数を初期スコープに登録
- [ ] テスト: Prelude関数が使えることを確認

### 1.4 Codegen対応
- [ ] Prelude関数のWASM生成
- [ ] 組み込み関数との連携

---

## Phase 2: Import解決

**Goal**: `import module.{name}` でモジュールから関数を取り込める

### 2.1 ModuleResolver統合
- [ ] main.rsにModuleResolver統合
- [ ] 検索パス設定 (., ./std, ~/.restrict/lib)
- [ ] モジュールファイル探索ロジック

### 2.2 Export収集
- [ ] パース時にexport宣言を収集
- [ ] エクスポートテーブル構築
- [ ] private関数のフィルタリング

### 2.3 Import処理
- [ ] import文のパース (既存)
- [ ] モジュールパス → ファイルパス解決
- [ ] インポートされた名前をスコープに追加
- [ ] 名前衝突の検出とエラー

### 2.4 型チェッカー統合
- [ ] インポートされた関数の型情報取得
- [ ] インポートされたRecord型の登録
- [ ] インポートされたContext型の登録

### 2.5 循環依存検出
- [ ] 依存グラフ構築
- [ ] 循環検出アルゴリズム
- [ ] 明確なエラーメッセージ

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
