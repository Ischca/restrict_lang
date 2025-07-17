# Lifetime Parameter Naming: More Intuitive Options

## The Problem

`'t` represents "how long a resource is valid" - we need a name that makes this immediately clear.

## More Intuitive Options

### 1. **"Lifetime"** (素直に)
```restrict
record File<'lifetime> { ... }
// "Fileのlifetime（生存期間）は'lifetime"
```
- ✅ 最も正確
- ✅ 他の言語でも使われている
- ❌ 長い

### 2. **"Live"** (短縮形)
```restrict
record File<'live> { ... }
// "Fileが生きている期間'live"
```
- ✅ 短い
- ✅ 直感的
- ✅ "live until" "live within" が自然

### 3. **"Valid"**
```restrict
record File<'valid> { ... }
// "Fileが有効な期間'valid"
```
- ✅ プログラマーに馴染み深い
- ✅ "valid until" "valid within" が自然
- ✅ 意味が明確

### 4. **"Life"**
```restrict
record File<'life> { ... }
// "Fileのlife（寿命）"
```
- ✅ 短くて分かりやすい
- ✅ 生物的メタファーで直感的
- ❌ カジュアルすぎる？

### 5. **"Use"**
```restrict
record File<'use> { ... }
// "Fileを使える期間'use"
```
- ✅ 実用的
- ✅ "use within" が自然
- ❌ 動詞っぽい

### 6. **"Active"**
```restrict
record File<'active> { ... }
// "Fileがアクティブな期間"
```
- ✅ 状態を表す
- ✅ プログラマーに馴染み深い
- ❌ 少し長い

## 日本語での説明を考えると

```restrict
record File<'t> { ... }
```

- Lifetime → 「生存期間」
- Live → 「生存」
- Valid → 「有効期間」
- Life → 「寿命」
- Use → 「使用期間」
- Active → 「活性期間」

## Recommendation: **"Life"** または **"Valid"**

### Option A: "Life" (カジュアルで親しみやすい)
```restrict
record File<'life> {
    handle: FileHandle
}

record Transaction<'tx, 'db> where 'tx within 'db {
    // "Transaction's life 'tx is within Database's life 'db"
    conn: Connection<'db>
}

// エラーメッセージ
"Error: Cannot return value with life 'conn outside its context"
"Error: Life 'tx must be within life 'db"
```

### Option B: "Valid" (技術的で正確)
```restrict
record File<'valid> {
    handle: FileHandle  
}

record Transaction<'tx, 'db> where 'tx within 'db {
    // "Transaction is valid for 'tx within Database's validity 'db"
    conn: Connection<'db>
}

// エラーメッセージ
"Error: Cannot return value valid for 'conn outside its context"
"Error: Validity 'tx must be within validity 'db"
```

## 実際の使用感

### "Life"の場合
```restrict
// "FileのlifeはFileSystemのlifeに束縛される"
with FileSystem {
    FileSystem.open("data.txt") { file ->
        // file's life is bound to FileSystem's life
    }
}
```

### "Valid"の場合
```restrict
// "Fileが有効なのはFileSystemが有効な間"
with FileSystem {
    FileSystem.open("data.txt") { file ->
        // file is valid while FileSystem is valid
    }
}
```

## 結論

個人的には **"life"** が良いと思います：
- 短い（4文字）
- 直感的（「寿命」「生存期間」）
- 親しみやすい
- "life 'tx within life 'db" が自然に読める

どう思いますか？