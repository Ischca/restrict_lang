// Context Binding — Scoped Capabilities
//
// Contexts provide implicit parameters to functions,
// similar to Scala's 'given'/'using' or Kotlin's context receivers.
// A function declares required contexts with 'with',
// and the caller provides them via 'with ContextName { ... } { body }'.

record QueryRequest { id: Int32 }

context Database {
    connection_id: Int32
}

fun main: () -> Int32 = {
    with Database { connection_id: 100 } {
        val request = QueryRequest { id: 7 }
        request.id + connection_id
    }
}
