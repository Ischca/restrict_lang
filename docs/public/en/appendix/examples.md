# Examples

These examples are small, self-contained programs that fit the v0.0.1 release
surface. They avoid host stdin, filesystem access, networking, Temporal Affine
Types, user-defined ADTs, and composite host exports.

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
