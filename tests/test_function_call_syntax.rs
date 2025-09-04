//! Test that traditional function call syntax is NOT supported
//! Only OSV (Object-Subject-Verb) syntax should be accepted

#[cfg(test)]
mod tests {
    use restrict_lang::parse_program;

    #[test]
    fn test_traditional_function_call_rejected() {
        // Traditional function call syntax should NOT parse
        let traditional_syntax = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = {
                x + y
            }
            
            fun main: () = {
                val result = add(5, 10);  // This should FAIL
                result
            }
        "#;
        
        let result = parse_program(traditional_syntax);
        assert!(result.is_err(), "Traditional function call syntax should be rejected");
    }
    
    #[test]
    fn test_osv_function_call_accepted() {
        // OSV function call syntax should parse correctly
        let osv_syntax = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = {
                x + y
            }
            
            fun main: () = {
                val result = (5, 10) add;  // OSV syntax - should work
                result
            }
        "#;
        
        let result = parse_program(osv_syntax);
        assert!(result.is_ok(), "OSV function call syntax should be accepted");
    }
    
    #[test]
    fn test_pipe_operator_accepted() {
        // Pipe operator should work
        let pipe_syntax = r#"
            fun double: (x: Int32) -> Int32 = {
                x * 2
            }
            
            fun main: () = {
                val result = 5 |> double;  // Pipe operator - should work
                result
            }
        "#;
        
        let result = parse_program(pipe_syntax);
        assert!(result.is_ok(), "Pipe operator syntax should be accepted");
    }
    
    #[test]
    fn test_no_args_function_call() {
        // No-argument function calls
        let no_args_osv = r#"
            fun get_value: () -> Int32 = {
                42
            }
            
            fun main: () = {
                val result = () get_value;  // OSV with no args
                result
            }
        "#;
        
        let result = parse_program(no_args_osv);
        assert!(result.is_ok(), "No-argument OSV function call should be accepted");
        
        // Traditional no-args syntax should fail
        let no_args_traditional = r#"
            fun get_value: () -> Int32 = {
                42
            }
            
            fun main: () = {
                val result = get_value();  // Traditional - should FAIL
                result
            }
        "#;
        
        let result = parse_program(no_args_traditional);
        assert!(result.is_err(), "Traditional no-args function call should be rejected");
    }
    
    #[test]
    fn test_print_function_calls() {
        // Test print functions use OSV syntax
        let print_osv = r#"
            fun main: () = {
                "Hello World" |> println;         // Pipe - should work
                ("Message: ", 42) print;          // OSV - should work
                42 |> print_int;                  // Pipe - should work
            }
        "#;
        
        let result = parse_program(print_osv);
        assert!(result.is_ok(), "Print functions with OSV syntax should work");
        
        // Traditional print calls should fail
        let print_traditional = r#"
            fun main: () = {
                println("Hello World");  // Traditional - should FAIL
            }
        "#;
        
        let result = parse_program(print_traditional);
        assert!(result.is_err(), "Traditional print function call should be rejected");
    }
    
    #[test]
    fn test_nested_function_calls() {
        // Nested OSV calls
        let nested_osv = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
            fun double: (x: Int32) -> Int32 = { x * 2 }
            
            fun main: () = {
                val result = (5, 10) add |> double;  // Chained with pipe
                val result2 = ((2, 3) add) double;   // Nested OSV
                result + result2
            }
        "#;
        
        let result = parse_program(nested_osv);
        assert!(result.is_ok(), "Nested OSV function calls should work");
        
        // Nested traditional calls should fail
        let nested_traditional = r#"
            fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
            fun double: (x: Int32) -> Int32 = { x * 2 }
            
            fun main: () = {
                val result = double(add(5, 10));  // Traditional nested - should FAIL
                result
            }
        "#;
        
        let result = parse_program(nested_traditional);
        assert!(result.is_err(), "Nested traditional function calls should be rejected");
    }
}