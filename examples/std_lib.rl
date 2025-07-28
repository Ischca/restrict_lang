// Standard Library for Restrict Language
// Provides basic functionality for common operations

// String operations
impl String {
    fun println = |self: String| {
        // This would be implemented as a built-in
        __builtin_println(self)
    }
    
    fun print = |self: String| {
        __builtin_print(self)
    }
    
    fun length = |self: String| -> Int32 {
        __builtin_string_length(self)
    }
    
    fun concat = |self: String, other: String| -> String {
        __builtin_string_concat(self, other)
    }
    
    // Operator overload for ++
    fun ++ = |self: String, other: String| -> String {
        self other.concat
    }
    
    fun toString = |self: String| -> String {
        self
    }
}

// Int32 operations
impl Int32 {
    fun toString = |self: Int32| -> String {
        __builtin_int_to_string(self)
    }
    
    fun println = |self: Int32| {
        self.toString.println
    }
    
    fun toFloat = |self: Int32| -> Float64 {
        __builtin_int_to_float(self)
    }
}

// Float64 operations
impl Float64 {
    fun toString = |self: Float64| -> String {
        __builtin_float_to_string(self)
    }
    
    fun println = |self: Float64| {
        self.toString.println
    }
}

// Bool operations
impl Bool {
    fun toString = |self: Bool| -> String {
        if self { "true" } else { "false" }
    }
    
    fun println = |self: Bool| {
        self.toString.println
    }
    
    fun not = |self: Bool| -> Bool {
        if self { false } else { true }
    }
}

// List operations
impl<T> List<T> {
    fun length = |self: List<T>| -> Int32 {
        __builtin_list_length(self)
    }
    
    fun head = |self: List<T>| -> Option<T> {
        match self {
            [] -> None,
            [x, ...rest] -> Some(x)
        }
    }
    
    fun tail = |self: List<T>| -> List<T> {
        match self {
            [] -> [],
            [x, ...rest] -> rest
        }
    }
    
    fun map = |self: List<T>, f: T -> U| -> List<U> {
        match self {
            [] -> [],
            [x, ...rest] -> [x.f] ++ rest.map(f)
        }
    }
    
    fun filter = |self: List<T>, pred: T -> Bool| -> List<T> {
        match self {
            [] -> [],
            [x, ...rest] -> {
                if x.pred {
                    [x] ++ rest.filter(pred)
                } else {
                    rest.filter(pred)
                }
            }
        }
    }
    
    fun forEach = |self: List<T>, f: T -> Unit| {
        match self {
            [] -> (),
            [x, ...rest] -> {
                x.f;
                rest.forEach(f)
            }
        }
    }
    
    fun flatMap = |self: List<T>, f: T -> List<U>| -> List<U> {
        match self {
            [] -> [],
            [x, ...rest] -> x.f ++ rest.flatMap(f)
        }
    }
    
    // Operator overload for ++
    fun ++ = |self: List<T>, other: List<T>| -> List<T> {
        __builtin_list_concat(self, other)
    }
}

// Option operations
impl<T> Option<T> {
    fun map = |self: Option<T>, f: T -> U| -> Option<U> {
        match self {
            Some(x) -> Some(x.f),
            None -> None
        }
    }
    
    fun flatMap = |self: Option<T>, f: T -> Option<U>| -> Option<U> {
        match self {
            Some(x) -> x.f,
            None -> None
        }
    }
    
    fun getOrElse = |self: Option<T>, default: T| -> T {
        match self {
            Some(x) -> x,
            None -> default
        }
    }
    
    fun toString = |self: Option<T>| -> String {
        match self {
            Some(x) -> "Some(" ++ x.toString ++ ")",
            None -> "None"
        }
    }
}

// Unit operations
impl Unit {
    fun toString = |self: Unit| -> String {
        "()"
    }
    
    fun println = |self: Unit| {
        self.toString.println
    }
}

// Panic function
fun panic = |message: String| -> Never {
    message.println;
    __builtin_panic()
}

// Time functions
fun currentTimeMillis = || -> Float64 {
    __builtin_current_time_millis()
}

// Try-catch support (simplified)
fun try = |<T> f: () -> T, catch: Exception -> T| -> T {
    __builtin_try_catch(f, catch)
}