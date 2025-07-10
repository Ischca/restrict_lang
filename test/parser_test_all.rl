// Test all major language constructs

// Record declaration
record Point { x: Int y: Int }

// Context declaration  
context Logger {
    log_level: Int
}

// Simple function
fun add = a: Int b: Int { a + b }

// Function with no parameters
fun get_answer = { 42 }

// Variable bindings
val immutable_var = 10
mut val mutable_var = 20

// Record literal
val origin = Point { x = 0, y = 0 }

// Clone and freeze
val moved_point = origin.clone { x = 5 } freeze

// Implementation block
impl Point {
    fun distance = self: Point { 
        self.x * self.x + self.y * self.y
    }
}

// Block expression
val block_result = {
    val temp = 10
    temp + 5
}

// Then/else conditional
val cond_result = true then { 1 } else { 0 }

// While loop
val loop_result = {
    mut val i = 0
    i < 10 while {
        i = i + 1
    }
    i
}

// Pipe operator
val piped = 42 |> add 10

// With context
fun with_logging = @logger ctx: Logger {
    with (logger) {
        // Use logger context here
        42
    }
}

// Match expression (if supported)
// val match_result = x match {
//     0 => { "zero" }
//     _ => { "other" }
// }