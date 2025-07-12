// Hello World example in Restrict Language

fun main = {
    // Simple variable binding
    val message = "Hello, Restrict Language!";
    val number = 42;
    
    // Function call with OSV syntax
    val result = (number) double;
    result
}

// Function definition
fun double = x:Int -> Int {
    x * 2
}

// Lambda expressions
val add_one = |x| x + 1;
val multiply = |x, y| x * y;

// Higher-order function
fun apply_twice = f:Int->Int, x:Int -> Int {
    val once = (x) f;
    (once) f
}

// Pattern matching
fun describe_number = n:Int -> String {
    n match {
        0 => { "zero" }
        1 => { "one" }
        _ => { "some number" }
    }
}

// Record definition and usage
record Person {
    name: String,
    age: Int,
}

impl Person {
    fun greet = self:Person -> String {
        "Hello, " + self.name + "!"
    }
}

fun test_person = {
    val alice = Person { name: "Alice", age: 30 };
    val greeting = alice.greet();
    greeting
}