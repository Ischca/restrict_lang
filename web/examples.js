// Auto-generated from samples/ — do not edit manually.
// Run: ./scripts/sync_samples.sh
//
// Each key maps to the contents of the corresponding .rl file in samples/.

export const examples = {
    'hello': `// Hello World in Restrict Language
//
// The simplest program: a single expression piped to println.
// Restrict uses |> (pipe) to chain operations left-to-right.

fun main = {
    "Hello, World!" |> println
}`,

    'pipe': `// Pipe Operators & OSV Syntax
//
// Restrict uses Object-Subject-Verb word order:
//   "5 double" instead of "double(5)"
//
// The pipe operator |> makes data flow explicit
// and chains transformations left-to-right.

fun double: (n: Int) -> Int = {
    n * 2
}

fun add_one: (n: Int) -> Int = {
    n + 1
}

fun main = {
    // OSV syntax: object comes first, then the function
    val a = 5 double          // => 10

    // Chaining with pipes: read left-to-right
    val b = 10 |> double |> add_one  // 10 -> 20 -> 21
    b int_to_string |> println

    // Pipes compose naturally into a pipeline
    5 |> double |> double |> int_to_string |> println  // => "20"
}`,

    'affine': `// Affine Types — Use-at-most-once Semantics
//
// Every binding in Restrict can be used 0 or 1 times.
// This prevents aliasing bugs and enables safe memory management
// without a garbage collector.

fun greet: (name: String) -> String = {
    name
}

fun main = {
    val message = "World"

    // First use — OK, this consumes 'message'
    message greet |> println

    // Uncommenting the next line would cause a compile error:
    // message greet |> println  // Error: 'message' already consumed
}`,

    'mutable': `// Mutable Variables & Loops
//
// Use 'mut val' to declare a mutable binding.
// Loops use the 'while' keyword with OSV syntax:
//   condition while { body }

fun fizzbuzz: (n: Int) -> String = {
    n % 15 == 0 then { "FizzBuzz" } else {
        n % 3 == 0 then { "Fizz" } else {
            n % 5 == 0 then { "Buzz" } else {
                n int_to_string
            }
        }
    }
}

fun main = {
    mut val i = 1
    i <= 20 while {
        i fizzbuzz |> println
        i = i + 1
    }
}`,

    'record': `// Records & Arena Memory
//
// Records are value types with named fields.
// Heap allocation happens inside 'with Arena { ... }' blocks,
// ensuring memory is freed when the scope ends.
// Use .clone {} to create a copy — the original is consumed
// by clone (affine semantics).

record Point { x: Int, y: Int }

fun show_x: (p: Point) -> String = {
    p.x int_to_string
}

fun main = {
    with Arena {
        val p = Point { x = 3, y = 4 }
        p show_x |> println                // consumes p => "3"

        val q = Point { x = 10, y = 20 }
        val q2 = q.clone {}                // consumes q, returns a copy
        q2.y int_to_string |> println      // consumes q2 => "20"
    }
    // Arena memory is freed here
}`,

    'context': `// Context Binding — Scoped Capabilities
//
// Contexts provide implicit parameters to functions,
// similar to Scala's 'given'/'using' or Kotlin's context receivers.
// A function declares required contexts with 'with',
// and the caller provides them via 'with ContextName { ... } { body }'.

record Connection { id: Int }

context Database {
    val conn: Connection
}

fun query: (sql: String) -> String with Database = {
    sql
}

fun main = {
    with Arena {
        val conn = Connection { id = 42 }

        with Database { conn = conn } {
            "SELECT * FROM users" query |> println
        }
    }
}`,

    'match': `// Pattern Matching — FizzBuzz
//
// The same FizzBuzz logic rewritten with tuple pattern matching.
// Match the tuple (n % 3, n % 5) to avoid nested if-else.
//
// Compare with the if-else version in "Mutable & Loops".

fun fizzbuzz: (n: Int) -> String = {
    (n % 3, n % 5) match {
        (0, 0) => { "FizzBuzz" }
        (0, _) => { "Fizz" }
        (_, 0) => { "Buzz" }
        _      => { n int_to_string }
    }
}

fun main = {
    mut val i = 1
    i <= 20 while {
        i fizzbuzz |> println
        i = i + 1
    }
}`,

    'compose': `// Scope Composition — Multiple Contexts
//
// Functions can require multiple contexts.
// Nested 'with' blocks compose naturally:
// the inner block inherits all outer contexts.
// This replaces the "dependency injection" pattern
// with compile-time checked capabilities.

record Logger { level: Int }
record Config { debug: Int }

context Logging { val logger: Logger }
context AppConfig { val config: Config }

fun start_app: () with Logging, AppConfig = {
    "App started with logging and config" |> println
}

fun main = {
    with Arena {
        val log = Logger { level = 1 }
        val cfg = Config { debug = 0 }

        with Logging { logger = log } {
            with AppConfig { config = cfg } {
                start_app
            }
        }
    }
}`,

};
