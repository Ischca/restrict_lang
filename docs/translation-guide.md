# Translation Guide / 翻訳ガイド

This guide helps maintain synchronized documentation between English and Japanese versions.

このガイドは、英語版と日本語版のドキュメントの同期を維持するのに役立ちます。

## Directory Structure / ディレクトリ構造

```
docs/
├── en/               # English documentation
│   ├── introduction.md
│   ├── getting-started/
│   ├── guide/
│   └── reference/
├── ja/               # Japanese documentation (日本語ドキュメント)
│   ├── introduction.md
│   ├── getting-started/
│   ├── guide/
│   └── reference/
└── scripts/
    └── sync-translations.sh  # Translation sync tool
```

## Translation Workflow / 翻訳ワークフロー

### 1. Check Translation Status / 翻訳状況の確認

```bash
cd docs
./scripts/sync-translations.sh
```

This will show:
- ✅ Up-to-date translations / 最新の翻訳
- ⚠️ Outdated translations / 古い翻訳
- ❌ Missing translations / 欠落している翻訳

### 2. Translation Priority / 翻訳の優先順位

1. **High Priority / 高優先度**
   - introduction.md
   - getting-started/installation.md
   - getting-started/hello-world.md

2. **Medium Priority / 中優先度**
   - guide/syntax.md
   - guide/types.md
   - guide/warder.md

3. **Low Priority / 低優先度**
   - reference/*
   - advanced/*

### 3. Translation Guidelines / 翻訳ガイドライン

#### Code Examples / コード例

Keep code examples identical between versions:
コード例は両バージョンで同一に保つ：

```restrict
// Same in both EN and JA
fn main() {
    "Hello, World!" |> println
}
```

Only translate comments:
コメントのみ翻訳：

```restrict
// EN: Calculate the sum
// JA: 合計を計算
let sum = a + b
```

#### Technical Terms / 技術用語

| English | Japanese | Notes |
|---------|----------|-------|
| Affine Type | アフィン型 | |
| Pipe Operator | パイプ演算子 | |
| Pattern Matching | パターンマッチング | |
| Ownership | 所有権 | |
| Prototype | プロトタイプ | |
| Generic | ジェネリック | |
| Trait | トレイト | |
| Implementation | 実装 | |

#### OSV Syntax Explanation / OSV構文の説明

When explaining OSV:
- EN: "Object-Subject-Verb word order"
- JA: "目的語-主語-動詞の語順"

### 4. Automated Checks / 自動チェック

Add to `.mise.toml`:

```toml
[tasks.doc-check-translations]
description = "Check translation synchronization"
run = "cd docs && ./scripts/sync-translations.sh"

[tasks.doc-update-ja]
description = "Update Japanese translations"
run = """
cd docs
echo "Files that need translation updates:"
./scripts/sync-translations.sh | grep -E "(Missing|Outdated)"
"""
```

### 5. Git Workflow / Gitワークフロー

When updating documentation:
ドキュメントを更新する際：

1. Update English version first / まず英語版を更新
2. Run translation check / 翻訳チェックを実行
3. Update Japanese version / 日本語版を更新
4. Commit both together / 両方を一緒にコミット

```bash
git add docs/en/guide/syntax.md docs/ja/guide/syntax.md
git commit -m "docs: Update syntax guide (EN/JA)"
```

### 6. Translation Memory / 翻訳メモリ

Common phrases / よく使うフレーズ:

| English | Japanese |
|---------|----------|
| "This guide covers..." | "このガイドでは...を説明します" |
| "In this section..." | "このセクションでは..." |
| "For example:" | "例：" |
| "Note that..." | "注意：..." |
| "See also:" | "参照：" |
| "Coming soon!" | "近日公開！" |

### 7. Quality Checklist / 品質チェックリスト

Before committing translations:
翻訳をコミットする前に：

- [ ] Code examples are identical / コード例が同一
- [ ] Technical terms are consistent / 技術用語が一貫している
- [ ] Links are updated / リンクが更新されている
- [ ] Formatting matches / フォーマットが一致
- [ ] No untranslated sections / 未翻訳のセクションがない

## Maintaining Synchronization / 同期の維持

### Using CI/CD / CI/CDの使用

Add to GitHub Actions:

```yaml
name: Check Translations
on: [pull_request]

jobs:
  check-translations:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Check translation sync
        run: |
          cd docs
          ./scripts/sync-translations.sh
```

### Translation Status Badge / 翻訳ステータスバッジ

Add to README.md:

```markdown
![Translation Status](https://img.shields.io/badge/translations-90%25-yellow)
```

## Contributing Translations / 翻訳への貢献

1. Check current status / 現在の状況を確認
2. Pick an outdated/missing file / 古い/欠落しているファイルを選択
3. Translate following guidelines / ガイドラインに従って翻訳
4. Submit PR with both versions / 両バージョンでPRを提出

## Tools and Resources / ツールとリソース

- [DeepL](https://www.deepl.com/) - For initial translations / 初期翻訳用
- [Google Translate](https://translate.google.com/) - For verification / 検証用
- VSCode Extensions:
  - [Markdown Preview Enhanced](https://marketplace.visualstudio.com/items?itemName=shd101wyy.markdown-preview-enhanced)
  - [Japanese Language Pack](https://marketplace.visualstudio.com/items?itemName=MS-CEINTL.vscode-language-pack-ja)

## Questions? / 質問は？

File an issue with the `translation` label.
`translation`ラベルを付けてissueを作成してください。