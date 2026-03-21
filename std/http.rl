// Restrict Language Standard Library: HttpClient Context
// 標準ライブラリ: HttpClient コンテキスト
//
// Provides HTTP client access using the context mechanism.
// Currently a stub - awaiting WASI-http support.
//
// Usage:
//   with HttpClient {
//       // HTTP operations will be available here
//   }

context HttpClient {
    get: (String) -> String
    post: (String, String) -> String
}
