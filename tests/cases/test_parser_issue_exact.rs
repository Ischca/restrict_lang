use restrict_lang::parse_program;

#[test]
fn test_exact_parser_issue() {
    let input = r#"record File {
    handle: Int32
}

fun openFile: () -> File = {
    val file = File { handle: 1 };  // file: File
    file
}

fun main: () -> Int32 = {
    val file = () openFile;
    file.handle
}"#;

    match parse_program(input) {
        Ok((remaining, program)) => {
            assert!(remaining.trim().is_empty());
            assert_eq!(program.declarations.len(), 3);
        }
        Err(e) => {
            panic!("Parse completely failed: {:?}", e);
        }
    }
}
