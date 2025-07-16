# Documentation Maintenance Rules

This document establishes rules for maintaining documentation consistency across languages.

## 🌐 Multi-language Documentation Policy

### Primary Language
- **English (en)** is the primary documentation language
- All new features MUST be documented in English first
- English documentation serves as the source of truth

### Translation Requirements
- **Japanese (ja)** documentation MUST be updated when English documentation changes
- Translation updates should happen in the same PR as the English changes when possible
- If immediate translation is not possible, create a follow-up issue

## 📝 Documentation Update Checklist

When making changes to the codebase that affect user-facing features:

1. **Update English documentation first**
   - [ ] Update relevant files in `docs/en/`
   - [ ] Update examples if needed
   - [ ] Update API references if applicable

2. **Update Japanese documentation**
   - [ ] Update corresponding files in `docs/ja/`
   - [ ] Ensure technical terms are consistently translated
   - [ ] Preserve code examples as-is (do not translate code)

3. **Update root documentation**
   - [ ] Update `README.md` if feature is significant
   - [ ] Update `CHANGELOG.md` with the change

## 🔍 Documentation Structure

Both `docs/en/` and `docs/ja/` should maintain parallel structure:

```
docs/
├── en/
│   ├── guide/
│   │   ├── patterns.md
│   │   ├── records.md
│   │   └── ...
│   └── reference/
│       └── ...
└── ja/
    ├── guide/
    │   ├── patterns.md    # Same filename as en/
    │   ├── records.md     # Same filename as en/
    │   └── ...
    └── reference/
        └── ...
```

## 🚨 Critical Documentation Areas

These areas MUST be kept in sync across all languages:

1. **Syntax changes** - Any change to language syntax
2. **Breaking changes** - Any change that breaks existing code
3. **New features** - Any new language feature or capability
4. **Security updates** - Any security-related changes

## 📋 Translation Guidelines

### DO:
- ✅ Keep technical terms consistent across documents
- ✅ Preserve all code examples exactly as they appear
- ✅ Maintain the same document structure and headings
- ✅ Update cross-references and links to point to correct language versions
- ✅ Keep formatting (bold, italic, code blocks) consistent

### DON'T:
- ❌ Translate variable names or code syntax
- ❌ Change the meaning or skip sections
- ❌ Add language-specific content without updating other languages
- ❌ Use machine translation without review

## 🔄 Review Process

1. **Code Review** - Reviewers should check that documentation is updated
2. **Translation Review** - Native speakers should review translations when possible
3. **Consistency Check** - Ensure parallel structure between language versions

## 📊 Documentation Status Tracking

Use these markers in commit messages and PRs:

- `[docs:en]` - English documentation updated
- `[docs:ja]` - Japanese documentation updated  
- `[docs:sync]` - Documentation synchronized across languages
- `[docs:todo-ja]` - Japanese translation needed (create issue)

## 🚀 Future Automation

Goals for CI/CD integration:
- Automated detection of documentation drift between languages
- Translation status badges
- Automated translation suggestions (with human review)
- Documentation coverage reports

## 📌 Quick Reference

**When you change code that affects users:**
1. Document in English (`docs/en/`)
2. Document in Japanese (`docs/ja/`)
3. Update README if needed
4. Ensure examples work

**Remember:** Documentation is part of the feature. A feature is not complete until it's documented in all supported languages.