# Temporal Type Variables with Backticks

## Backtick Syntax `` `t ``

```restrict
record File<`f> {
    handle: FileHandle
}

record Transaction<`tx, `db> where `tx within `db {
    conn: Connection<`db>
    state: TxState
}

fun processFile<`io> = file: File<`io> {
    file.read() |> transform |> file.write
}
```

## Advantages of Backticks

### 1. **Clean and Minimal**
```restrict
// Very light visual weight
Connection<`conn>
Transaction<`tx>
File<`f>
```

### 2. **Suggests "Quoting Time"**
- Backticks often mean "special interpretation"
- Like "quoting" a specific moment in time
- `` `now `` = "at this moment"

### 3. **No Semantic Conflicts**
- Not used for operators
- Not used for literals
- Clear meaning: temporal variable

### 4. **Good Readability**
```restrict
// Easy to spot but not intrusive
fun transfer<`tx, `db> = 
    from: Account<`tx> 
    to: Account<`tx>
    amount: Money
where `tx within `db {
    // transaction logic
}
```

## Comparison: Backtick vs Tilde

### Backtick `` ` ``
```restrict
record Cache<T, `valid> {
    data: T
    expiry: Time<`valid>
}

// Lighter, more subtle
where `tx within `db
```

### Tilde `~`
```restrict
record Cache<T, ~valid> {
    data: T
    expiry: Time<~valid>
}

// More visible, wave-like
where ~tx within ~db
```

## Real Usage Examples

### With Contexts
```restrict
with Database {
    Database.connect { conn ->      // conn: Connection<`db>
        conn.beginTx { tx ->        // tx: Transaction<`tx>
            // `tx within `db
            tx.execute("UPDATE ...");
            tx.commit();
        }
    }
}
```

### With OSV Syntax
```restrict
fun fetchAndProcess<`http> = urls: List<String> {
    urls
    |> map(|url| (url) http.get)    // List<Response<`http>>
    |> Future.all
    |> await
}
```

### Error Messages
```
Error: Temporal variable `tx must be within `db
Error: Cannot return value with temporal `conn outside its scope
Error: Temporal `session has expired
```

## Potential Issues

### 1. **Visibility**
- ❓ Might be too subtle
- ❓ Could be missed when scanning code

### 2. **Font Rendering**
- ❓ Some fonts make backticks hard to see
- ❓ Confusion with straight quotes in some editors

### 3. **Markdown/Documentation**
- ❓ Conflicts with markdown code syntax
- ❓ Need escaping in documentation

## Final Comparison

| Syntax | Example | Pros | Cons |
|--------|---------|------|------|
| `` `t `` | `File<`f>` | Clean, minimal, "quoting time" | Too subtle?, markdown conflicts |
| `~t` | `File<~f>` | Visible, wave metaphor, unique | More visual weight |
| `'t` | `File<'f>` | Standard (Rust), familiar | Conflicts with char literals |

## Recommendation

Both **backtick** and **tilde** are good choices:

- **Use backtick `` ` ``** if you prefer:
  - Minimal visual impact
  - Clean, subtle syntax
  - "Quoting time" metaphor

- **Use tilde `~`** if you prefer:
  - Better visibility
  - Wave/temporal metaphor
  - No documentation conflicts

## My Opinion

I slightly prefer **tilde `~`** because:
1. Better visibility in all fonts
2. No markdown conflicts
3. The wave metaphor fits temporality
4. Still just one character

But backticks are also elegant! What's your preference?

```restrict
// Backtick version
record Session<`s> {
    conn: Connection<`s>
}

// Tilde version  
record Session<~s> {
    conn: Connection<~s>
}
```