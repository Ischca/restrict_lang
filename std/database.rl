// Restrict Language Standard Library: Database Context
// 標準ライブラリ: Database コンテキスト
//
// Provides database access using the context mechanism.
// Currently a stub - awaiting WASI-sql support.
//
// Usage:
//   with Database {
//       // Database operations will be available here
//   }

context Database {
    connect: (String) -> Int
    query: (Int, String) -> String
    execute: (Int, String) -> Int
}
