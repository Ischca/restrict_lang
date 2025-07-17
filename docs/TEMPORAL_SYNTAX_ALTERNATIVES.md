# Temporal Type Variable Syntax Alternatives

## The Problem

Single quotes `'` are already used for character literals in Restrict Language:
```restrict
val c = 'a'      // Character literal
val quote = '\'' // Escaped single quote
```

Using `'t` for temporal type variables would create parsing ambiguity.

## Alternative Syntax Options

### 1. **Tilde `~`**
```restrict
record File<~t> { ... }
record Transaction<~tx, ~db> where ~tx within ~db { ... }

fun process<~life> = file: File<~life> -> Result<~life>
```
- ✅ Visually distinct
- ✅ Not used elsewhere in Restrict
- ✅ Suggests "approximately" or "temporal"
- ❌ Might be hard to type on some keyboards

### 2. **At Sign `@`**
```restrict
record File<@t> { ... }
record Transaction<@tx, @db> where @tx within @db { ... }

fun process<@life> = file: File<@life> -> Result<@life>
```
- ✅ Already used for context parameters
- ✅ Suggests "at this time"
- ❌ Might confuse with context binding

### 3. **Dollar Sign `$`**
```restrict
record File<$t> { ... }
record Transaction<$tx, $db> where $tx within $db { ... }

fun process<$life> = file: File<$life> -> Result<$life>
```
- ✅ Common in other languages for special variables
- ✅ Easy to type
- ❌ Might look like template syntax

### 4. **Hash/Pound `#`**
```restrict
record File<#t> { ... }
record Transaction<#tx, #db> where #tx within #db { ... }

fun process<#life> = file: File<#life> -> Result<#life>
```
- ✅ Suggests "number" or "index" (time as index)
- ✅ Visually distinct
- ❌ Often used for preprocessor directives

### 5. **Percent `%`**
```restrict
record File<%t> { ... }
record Transaction<%tx, %db> where %tx within %db { ... }

fun process<%life> = file: File<%life> -> Result<%life>
```
- ✅ Not commonly used
- ✅ Available on all keyboards
- ❌ Might suggest modulo operation

### 6. **Backtick `` ` ``**
```restrict
record File<`t> { ... }
record Transaction<`tx, `db> where `tx within `db { ... }

fun process<`life> = file: File<`life> -> Result<`life>
```
- ✅ Used for special identifiers in some languages
- ✅ Visually light
- ❌ Hard to see
- ❌ Conflicts with markdown

### 7. **Exclamation `!`**
```restrict
record File<!t> { ... }
record Transaction<!tx, !db> where !tx within !db { ... }

fun process<!life> = file: File<!life> -> Result<!life>
```
- ✅ Suggests "important" or "special"
- ❌ Usually means negation
- ❌ Too emphatic

### 8. **Underscore Prefix `_`**
```restrict
record File<_t> { ... }
record Transaction<_tx, _db> where _tx within _db { ... }

fun process<_life> = file: File<_life> -> Result<_life>
```
- ✅ Common convention for special variables
- ✅ Easy to type
- ❌ Usually means "unused" variable

### 9. **No Prefix (Context Distinguishes)**
```restrict
record File<t> { ... }          // If lowercase, it's temporal
record List<T> { ... }          // If uppercase, it's type

record Transaction<tx, db> where tx within db { ... }
fun process<life> = file: File<life> -> Result<life>
```
- ✅ Clean syntax
- ✅ Convention-based (like Go's public/private)
- ❌ Less explicit
- ❌ Might be confusing

### 10. **Keyword Prefix**
```restrict
record File<time t> { ... }
record Transaction<time tx, time db> where tx within db { ... }

// Or shorter:
record File<when t> { ... }
record Transaction<when tx, when db> where tx within db { ... }
```
- ✅ Extremely clear
- ✅ No ambiguity
- ❌ Verbose

## Recommendation: **Tilde `~`**

I recommend using the tilde `~` for temporal type variables:

```restrict
// Clear visual distinction
record File<~f> {
    handle: FileHandle
}

// Natural to read
record Transaction<~tx, ~db> where ~tx within ~db {
    conn: Connection<~db>
    state: TxState
}

// Usage
fun copyFile<~io> = source: String dest: String {
    val input = fs.open(source);   // File<~io>
    val output = fs.create(dest);   // File<~io>
    
    input.read() |> output.write;
}

// With contexts
with Database {
    Database.connect { conn ->      // conn: Connection<~db>
        conn.beginTx { tx ->        // tx: Transaction<~tx>
            // ~tx within ~db
        }
    }
}
```

## Why Tilde?

1. **No conflicts** - Not used anywhere else in Restrict
2. **Meaningful** - Suggests "approximate time" or "wave" (temporal)
3. **Visible** - More visible than backtick, less loud than !
4. **Ergonomic** - Single character, available on most keyboards
5. **Distinct** - Can't be confused with regular type variables

## Convention

```restrict
// Type variables: uppercase letters
T, U, V, K         // Types

// Temporal variables: lowercase with ~
~t, ~u, ~v         // Generic temporal
~conn, ~tx, ~req   // Descriptive temporal
```

## Error Messages

```
Error: Temporal variable ~tx must be within ~db
Error: Cannot return value with temporal ~conn outside its scope
Error: Temporal variable ~a has expired
```

This gives us a clean, unambiguous syntax for temporal type variables!