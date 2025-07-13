// Standard IO functions for Restrict Language

// Print integer to stdout
export fun println = value:Int32 -> Unit {
    // This will be implemented as a built-in function
    @builtin("println_i32", value)
}

// Print string to stdout  
export fun print_string = value:String -> Unit {
    @builtin("print_string", value)
}

// Read line from stdin
export fun read_line = Unit -> String {
    @builtin("read_line")
}