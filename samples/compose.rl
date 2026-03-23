// Scope Composition — Multiple Contexts
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
}
