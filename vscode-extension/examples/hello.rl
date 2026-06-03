// Hello World example in Restrict Language

// Function definition
fun double: (x: Int32) -> Int32 = {
    x * 2
}

fun add_one: (x: Int32) -> Int32 = {
    x + 1
}

// Function composition
fun add_two: (x: Int32) -> Int32 = {
    val once = x |> add_one
    once |> add_one
}

// Pattern matching
fun describe_number: (n: Int32) -> String = {
    n match {
        0 => { "zero" }
        1 => { "one" }
        _ => { "some number" }
    }
}

// Record definition and usage
record Person {
    name: String
    age: Int32
}

fun greet: (self: Person) -> String = {
    "Hello, " + self.name + "!"
}

fun age_bonus: (self: Person) -> Int32 = {
    self.age >= 18 then { 10 } else { 0 }
}

fun greeting_for_alice: () -> String = {
    val alice = Person { name: "Alice", age: 30 }
    val greeting = alice |> greet
    greeting
}

fun main: () -> Int32 = {
    val number = 42
    val doubled = number |> double
    val transformed = doubled |> add_two

    val alice = Person { name: "Alice", age: 30 }
    val bonus = alice |> age_bonus

    transformed + bonus
}
