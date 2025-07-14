```restrict
use std::testing::*;

#[test]
fn test_greeting() {
    let greeting = createGreeting("World")
    greeting |> assertEquals("Hello, World!")
}

fn createGreeting(name: String) -> String {
    "Hello, " ++ name ++ "!"
}
```