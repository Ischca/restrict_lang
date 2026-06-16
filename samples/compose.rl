// Scope Composition — Multiple Contexts
//
// Functions can require multiple contexts.
// Nested 'with' blocks compose naturally:
// the inner block inherits all outer contexts.
// This replaces the "dependency injection" pattern
// with compile-time checked capabilities.

record Event { severity: Int32 }

context Logging {
    level: Int32
}

context AppConfig {
    debug: Int32
}

fun main: () -> Int32 = {
    with Logging { level: 10 } {
        with AppConfig { debug: 1 } {
            val event = Event { severity: 4 }
            event.severity + level + debug
        }
    }
}
