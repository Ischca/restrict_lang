```restrict
fun test_greeting: () -> () = {
    val greeting = "World" |> create_greeting
    (greeting == "Hello, World!", "greeting should include the name") assert
}

fun create_greeting: (name: String) -> String = {
    "Hello, " + name + "!"
}
```
