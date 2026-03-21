// Restrict Language Standard Library: FileSystem Context
// 標準ライブラリ: FileSystem コンテキスト
//
// Provides safe, scoped file I/O using the context mechanism.
// Uses WASI (WebAssembly System Interface) for underlying operations.
//
// Usage:
//   with FileSystem {
//       val content = "path/to/file" read_file_safe;
//       content match {
//           Ok(text) => { text println }
//           Err(code) => { "Error" println }
//       }
//   }

context FileSystem {
    open: (String, Int) -> Int
    read: (Int, Int) -> String
    write: (Int, String) -> Int
    close: (Int) -> Int
}

// Safe file reading with automatic close and error handling
export fun read_file_safe: (path: String) -> Result<String, Int> = {
    val fd = (path, 0) file_open;
    fd < 0 then {
        Err(fd)
    } else {
        val content = (fd, 4096) file_read;
        fd file_close;
        Ok(content)
    }
}

// Safe file writing with automatic close and error handling
export fun write_file_safe: (path: String, content: String) -> Result<Int, Int> = {
    val fd = (path, 1) file_open;
    fd < 0 then {
        Err(fd)
    } else {
        val bytes_written = (fd, content) file_write;
        fd file_close;
        Ok(bytes_written)
    }
}
