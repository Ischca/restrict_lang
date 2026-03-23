// Context Binding — Scoped Capabilities
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
}
