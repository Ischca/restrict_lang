// Temporal Types Example: File I/O with automatic cleanup
//
// This example demonstrates how temporal type variables ensure
// that resources are properly managed and cleaned up.

// File type with temporal parameter ~f
// The ~f represents "when" this file handle is valid
record File<~f> {
    handle: Int32  // Simplified file handle
    path: String
}

// FileSystem context that creates temporal scope
context FileSystem<~fs> {
    open: (String, (File<~fs>) -> Unit) -> Unit
    read: File<~fs> -> String
    write: (File<~fs>, String) -> Unit
}

// Database and Transaction with temporal constraints
record Database<~db> {
    connection: Int32
}

record Transaction<~tx, ~db> where ~tx within ~db {
    db: Database<~db>
    txId: Int32
}

// Function that reads a file within a temporal scope
fun readFileContent: <~io>(file: File<~io>) -> String = {
    // File is valid here because we're within ~io scope
    file.path |> println;
    "File content here"  // Simplified
}

// Function with temporal constraint
fun beginTransaction: <~db, ~tx>(db: Database<~db>) -> Transaction<~tx, ~db>
where ~tx within ~db = {
    Transaction { 
        db: db, 
        txId: 42  // Simplified transaction ID
    }
}

// Example of using temporals with callback pattern
fun processFile: (filename: String) = {
    with FileSystem {
        // FileSystem.open creates a File<~fs> that's only valid in callback
        FileSystem.open(filename) { file ->
            // file: File<~fs> is valid here
            val content = file |> FileSystem.read;
            content |> println;
            
            // File automatically cleaned up when callback ends
        }  // ~fs scope ends, file is cleaned up
    }
}

// Example of nested temporal scopes
fun databaseTransaction: () = {
    with Database {  // Creates ~db scope
        val db = Database { connection: 1 };
        
        // Transaction must be within database scope
        val tx = db |> beginTransaction;  // tx: Transaction<~tx, ~db>
        
        // Use transaction...
        "Processing transaction" |> println;
        
        // Transaction cleaned up before database
    }  // ~db scope ends, all resources cleaned up in correct order
}

// Main function demonstrating temporal types
fun main: () = {
    "=== Temporal Types Demo ===" |> println;
    
    // Process a file with automatic cleanup
    "test.txt" |> processFile;
    
    // Run a database transaction
    databaseTransaction;
    
    "All resources cleaned up!" |> println;
}