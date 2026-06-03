// Temporal Affine Types (TAT) Cleanup Demo
//
// Experimental design sketch: TAT is outside the default v0.0.1 release gate,
// so this file is not a runnable v0.0.1 release example.
//
// This example demonstrates the enhanced TAT implementation with automatic
// resource cleanup at temporal scope boundaries.

// File resource with temporal parameter ~f
record File<~f> {
    handle: Int32,
    path: String
}

// Database connection with temporal parameter ~db
record Database<~db> {
    connection: Int32,
    name: String
}

// Transaction that must be within database lifetime
record Transaction<~tx, ~db> where ~tx within ~db {
    db: Database<~db>,
    txId: Int32,
    active: Bool
}

// Logger for tracking cleanup operations
record Logger<~l> {
    name: String,
    level: Int32
}

// Function that processes a file within a temporal scope
fun processFile: (filename: String) -> Int32 = {
    with lifetime<~file_io> {
        // File is automatically registered for cleanup
        val file = File {
            handle: 42,
            path: filename
        };

        "Processing file: " ++ file.path |> println;

        // Simulate file operations
        val bytes_read = file.handle * 100;
        bytes_read
    }
    // File handle is automatically closed here via cleanup_file()
}

// Function that demonstrates database transaction cleanup
fun runDatabaseOperation: () -> Bool = {
    with lifetime<~database> {
        // Database connection established
        val db = Database {
            connection: 1001,
            name: "production"
        };

        "Connected to database: " ++ db.name |> println;

        with lifetime<~transaction> {
            // Transaction started within database scope
            val tx = Transaction {
                db: db,
                txId: 2001,
                active: true
            };

            "Started transaction: " |> println;
            tx.txId |> println;

            // Simulate database work
            val success = tx.active;

            if success {
                "Transaction completed successfully" |> println;
                true
            } else {
                "Transaction failed" |> println;
                false
            }
        }
        // Transaction is automatically rolled back/committed here via cleanup_transaction()

        "Database operations completed" |> println;
        true
    }
    // Database connection is automatically closed here via cleanup_database()
}

// Function demonstrating nested cleanup with mixed resource types
fun complexCleanupScenario: () -> Unit = {
    with lifetime<~outer> {
        val logger = Logger { name: "outer", level: 1 };
        "Starting complex scenario" |> println;

        val file1 = File { handle: 100, path: "outer.log" };

        with lifetime<~middle> {
            val db = Database { connection: 2002, name: "cache" };
            val file2 = File { handle: 200, path: "middle.tmp" };

            with lifetime<~inner> {
                val tx = Transaction {
                    db: db,
                    txId: 3003,
                    active: true
                };
                val file3 = File { handle: 300, path: "inner.data" };

                "All resources allocated" |> println;

                // Use all resources
                file1.handle + file2.handle + file3.handle + tx.txId;
                Unit
            }
            // file3 and tx cleaned up here (inner scope)

            "Inner scope cleaned up" |> println;
            Unit
        }
        // file2 and db cleaned up here (middle scope)

        "Middle scope cleaned up" |> println;
        Unit
    }
    // logger and file1 cleaned up here (outer scope)

    "All cleanup completed" |> println;
    Unit
}

// Main function demonstrating TAT cleanup
fun main: () -> Unit = {
    "=== Temporal Affine Types Cleanup Demo ===" |> println;

    // Demonstrate file cleanup
    val file_result = ("input.dat") processFile;
    "File processing result: " |> println;
    file_result |> println;

    // Demonstrate database transaction cleanup
    val db_success = () runDatabaseOperation;
    if db_success {
        "Database operations succeeded" |> println;
    } else {
        "Database operations failed" |> println;
    };

    // Demonstrate complex nested cleanup
    () complexCleanupScenario;

    "=== Demo completed - all resources cleaned up ===" |> println;
    Unit
}
