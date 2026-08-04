# Module System Roadmap

**Created**: 2025-01-11
**Last Updated**: 2026-08-04
**Status**: Phase 4 Complete, Phase 5 In Progress
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
- [x] 名前衝突の検出とエラー
- [x] split/direct/transitive import で canonical declaration identity を共有
- [x] 失敗した依存解決を cache せず retry 可能にする

### 2.4 型チェッカー統合 ✅
- [x] インポートされた関数の型情報取得 (register_imported_decl)
- [x] インポートされたRecord型の登録
- [x] インポートされたContext型の登録

### 2.5 循環依存検出 ✅
- [x] resolving set による依存追跡
- [x] 循環検出アルゴリズム
- [x] 完全な循環 chain を含むエラーメッセージ

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

## Phase 3: Codegen最適化 ⚠️ DEFERRED

**Goal**: 複数モジュールから最適化されたWASMを生成

**Status**: Deferred (基本機能は動作、最適化は将来)

### 3.1 モジュール収集
- [x] 使用されるモジュールの収集 (基本実装済み)
- [ ] 依存順序でのソート
- [ ] 未使用モジュールの除外 (dead code elimination)

### 3.2 名前マングリング
- [x] モジュール間での名前衝突回避
- [x] 長さ付き canonical internal name の生成規則
- [x] エクスポート名の保持

### 3.3 コード結合
- [x] 全モジュールのWASM関数を結合 (インライン展開)
- [ ] グローバル変数の統合
- [ ] メモリレイアウトの調整

### 3.4 最適化
- [ ] 未使用関数の削除
- [ ] インライン展開 (小さな関数)
- [ ] 定数畳み込み

---

## Phase 4: 標準ライブラリ整備 ✅ COMPLETED

**Goal**: 実用的な標準ライブラリを提供

**Status**: Completed on 2025-01-11

### Prerequisites ✅
- [x] Copy型サポート追加 (Int, Bool, Float, Char, Unitが複数回使用可能に)

### 4.1 std/io ✅ Built-in
- [x] print, println (polymorphic) - 組み込み関数として実装済み
- [ ] read_line (WASI依存)
- [ ] file operations (WASI依存)

### 4.2 std/list ✅ COMPLETED
- [x] is_empty, head, tail, length - 基本操作
- [x] prepend, concat, reverse - リスト構築
- [x] map, filter, fold - 高階関数
- [x] flatten - Option操作

### 4.3 std/option ✅ COMPLETED
- [x] is_some, is_none, unwrap_or - 基本操作

### 4.4 std/result ✅ COMPLETED (2025-01-11)
- [x] is_ok, is_err - 述語
- [x] unwrap_or, unwrap_err_or - 値取り出し
- [x] map_ok, map_err, and_then - 変換
- [x] ok, err - Option変換

### 4.5 std/string ✅ COMPLETED
- [x] string_length, string_concat, string_equals - WASM組み込み
- [x] char_at, substring - 文字アクセス
- [x] string_to_int, int_to_string - 変換
- [x] is_digit, is_alpha, is_whitespace - 文字分類
- [x] to_upper, to_lower - 文字変換
- [x] string utilities (is_empty, append, etc.)

### 4.6 std/math ✅ COMPLETED
- [x] abs, min, max, signum
- [x] is_positive, is_negative, is_zero
- [x] pow, gcd, lcm
- [x] clamp

### 4.7 std/prelude ✅ COMPLETED
- [x] not, identity functions
- [x] Comparison helpers
- [x] Arithmetic helpers

---

## Phase 5: パッケージマネージャ (Warder) 🚧 IN PROGRESS

**Goal**: サードパーティライブラリの配布と利用

**Status**: Basic structure implemented, some features incomplete

### 5.1 warder.toml設計 ✅
- [x] パッケージメタデータ形式 (package.rl.toml)
- [x] 依存関係記述
- [x] バージョン指定

### 5.2 プロジェクト管理 ✅
- [x] `warder new <name>` - 新規プロジェクト作成
- [x] `warder init` - 現在のディレクトリで初期化
- [x] `warder doctor` - プロジェクト診断

### 5.3 ビルドシステム ⚠️ PARTIAL
- [x] `warder build` - 基本ビルド
- [x] `warder run` - wasmtime/wasmer で実行
- [ ] `warder build --watch` - ファイル監視 (未実装)
- [ ] `warder build --component` - WASM Component (部分実装)

### 5.4 テスト ⚠️ PARTIAL
- [x] `warder test` - テストファイル検出
- [ ] テストランナー実装 (スケルトンのみ)

### 5.5 依存関係管理 ⚠️ PARTIAL
- [x] `warder add <dep>` - 依存追加 (基本構造)
- [x] restrict-lock.toml - ロックファイル
- [ ] 依存解決アルゴリズム (TODO)
- [ ] レジストリからのフェッチ (TODO)

### 5.6 Cage フォーマット ✅
- [x] `warder wrap` - WASMをCageにパッケージ
- [x] `warder unwrap` - Cageから展開
- [x] ABI hash計算

### 5.7 パッケージ公開 ❌ NOT IMPLEMENTED
- [ ] `warder publish` - レジストリへ公開
- [ ] WardHub レジストリ
- [ ] sigstore 署名

---

## 残タスクまとめ

### 高優先度
| タスク | 説明 | 状態 |
|--------|------|------|
| 名前衝突検出 | 同名インポート時のエラー | TODO |
| 修飾名アクセス | `std.math.abs` 構文 | TODO |

### 中優先度 (Warder)
| タスク | 説明 | 状態 |
|--------|------|------|
| 依存解決 | 完全な依存解決アルゴリズム | TODO |
| テストランナー | 実際のテスト実行 | TODO |
| Watch mode | ファイル監視ビルド | TODO |

### 低優先度
| タスク | 説明 | 状態 |
|--------|------|------|
| Re-exports | `export import module.*` | TODO |
| Dead code elimination | 未使用関数削除 | TODO |
| パッケージ公開 | WardHub連携 | TODO |
| WASI対応 | read_line, ファイル操作 | TODO |

---

## Success Metrics

### Phase 1-4 完了条件 ✅ ALL PASSED
- [x] `42 print` がPreludeインポートなしで動作
- [x] `import math.{abs}` で関数をインポート可能
- [x] 循環依存でエラー
- [x] 未エクスポート関数へのアクセスでエラー
- [x] std/math: abs, min, max, pow, gcd, lcm, clamp 実装
- [x] std/option: is_some, is_none, unwrap_or 実装
- [x] std/list: map, filter, fold 実装
- [x] std/result: is_ok, is_err, map_ok, and_then 実装

### Phase 5 完了条件 🚧 IN PROGRESS
- [x] `warder new/init` でプロジェクト作成
- [x] `warder build/run` でビルド・実行
- [ ] 依存関係の自動解決
- [ ] サードパーティライブラリを作成・利用可能

---

## Notes

- 各Phaseは独立してテスト可能にする
- 後方互換性を維持する
- エラーメッセージは常に明確に

---

*Last updated: 2025-01-11*
