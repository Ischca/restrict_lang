# Examples

These examples are small, self-contained programs. Most fit the historical
v0.0.1 release surface; the enum example is labeled as a current post-v0.0.1
addition. They avoid host stdin, filesystem access, networking, Temporal Affine
Types, and composite host exports.

## Scalar Pipeline

```restrict
fun add_bonus: (base: Int32, bonus: Int32) -> Int32 = {
    base + bonus
}

fun clamp: (score: Int32) -> Int32 = {
    score > 100 then {
        100
    } else {
        score
    }
}

fun main: () -> Int32 = {
    (96, 7) add_bonus |> clamp
}
```

## Record Computation

Use records internally and expose scalar summaries when crossing the host
boundary.

```restrict
record ReleaseCheck {
    tests: Int32,
    failures: Int32
}

fun passed_tests: (check: ReleaseCheck) -> Int32 = {
    val ReleaseCheck { tests, failures } = check
    tests - failures
}

export fun exported_passed_tests: () -> Int32 = {
    val check = ReleaseCheck { tests: 42, failures: 2 }
    check |> passed_tests
}
```

## Option Handling

```restrict
fun score_or_default: (score: Option<Int32>) -> Int32 = {
    score match {
        Some(value) => { value }
        None => { 0 }
    }
}

fun main: () -> Int32 = {
    Some(42) |> score_or_default
}
```

`None` often needs type context from a function parameter, return type,
annotation, or sibling branch.

## Result Handling

```restrict
fun checked_divide: (left: Int32, right: Int32) -> Result<Int32, String> = {
    right == 0 then {
        Err("division by zero")
    } else {
        Ok(left / right)
    }
}

fun result_or_zero: (value: Result<Int32, String>) -> Int32 = {
    value match {
        Ok(result) => { result }
        Err(message) => { 0 }
    }
}

fun main: () -> Int32 = {
    (84, 2) checked_divide |> result_or_zero
}
```

## Custom Enum Error In Result

This example requires the current post-v0.0.1 compiler. It uses a closed,
non-generic, non-recursive enum, qualified OSV construction, and exhaustive
matching. Running it prints `-2`.

```restrict
enum CustomError {
    Empty
    Invalid(String)
}

fun decode: (code: Int32) -> Result<Int32, CustomError> = {
    code == 0 then {
        Ok(42)
    } else {
        Err("invalid code" |> CustomError::Invalid)
    }
}

fun collapse: (result: Result<Int32, CustomError>) -> Int32 = {
    result match {
        Ok(value) => { value }
        Err(error) => {
            error match {
                CustomError::Empty => { -1 }
                CustomError::Invalid(message) => { -2 }
            }
        }
    }
}

fun main: () -> () = {
    1 |> decode |> collapse |> print_int
}
```

`pub enum` can expose the enum namespace to another Restrict source module, but
there is no direct host enum ABI. Generic and recursive enums and postfix `?`
propagation remain future work.

## Lambda Context

Lambdas infer parameter types from the expected function type:

```restrict
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    (|value| value + 1, 41) apply_int
}
```

When no expected type exists, annotate the lambda parameter:

```restrict
fun main: () -> Int32 = {
    val bump = |value: Int32| value + 1
    41 |> bump
}
```

## Host Export Wrapper

`main` is for program execution. Add a separate scalar export when a host needs
to call a function and read its return value:

```restrict
fun compute_score: () -> Int32 = {
    42
}

export fun exported_score: () -> Int32 = {
    () compute_score
}

fun main: () -> Int32 = {
    () compute_score
}
```
