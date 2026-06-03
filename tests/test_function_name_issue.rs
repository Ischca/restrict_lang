use restrict_lang::parse_program;

#[test]
#[ignore = "TAT (Temporal Affine Types) syntax - deferred to v2.0"]
fn test_different_function_names() {
    let names = vec![
        "test",
        "foo",
        "myFunc",
        "leak",
        "leakF",
        "leakFi",
        "leakFil",
        "leakFile",
        "leakFiles",
        "fileLeaker",
    ];

    for name in names {
        let input = format!("fun {} = {{ () }}", name);
        let (remaining, program) =
            parse_program(&input).unwrap_or_else(|e| panic!("{} should parse: {:?}", name, e));

        assert!(remaining.trim().is_empty());
        assert_eq!(program.declarations.len(), 1);
    }
}

#[test]
#[ignore = "TAT (Temporal Affine Types) syntax - deferred to v2.0"]
fn test_leakfile_variations() {
    let cases = [
        "fun leakFile = { () }",
        "fun leakFile: () -> () = { () }",
        "fun leakFile: (handle: Int32) -> Int32 = { handle }",
    ];

    for input in cases {
        let (remaining, program) =
            parse_program(input).unwrap_or_else(|e| panic!("{} should parse: {:?}", input, e));

        assert!(remaining.trim().is_empty());
        assert_eq!(program.declarations.len(), 1);
    }
}
