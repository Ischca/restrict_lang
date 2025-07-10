// Define contexts
context Database { 
    host: String 
    port: Int 
}

context Logger {
    level: String
}

// Function that requires Database context
fun connect = @Database url: String {
    val host = Database.host
    val port = Database.port
    42
}

// Using contexts
val result = with Database {
    val conn = "mydb" connect
    conn
}

// Multiple contexts
val app_result = with (Database, Logger) {
    val level = Logger.level
    val conn = "appdb" connect
    conn + 100
}