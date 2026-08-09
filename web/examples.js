// Auto-generated from samples/playground/manifest.json.
// Run: bash scripts/sync_samples.sh

export const exampleGroups = [
    {
        "id": "start",
        "title": "Start here"
    },
    {
        "id": "current",
        "title": "Current compiler"
    },
    {
        "id": "diagnostics",
        "title": "Diagnostics"
    }
];

export const examples = [
    {
        "id": "hello",
        "title": "Hello output",
        "group": "start",
        "description": "Run one expression and see its output immediately.",
        "file": "hello.rl",
        "kind": "run",
        "expectedOutput": "Hello, Restrict!\n",
        "source": "// Start with visible program output.\nfun main: () = {\n    \"Hello, Restrict!\" println\n}\n"
    },
    {
        "id": "functionsOsv",
        "title": "Functions and OSV",
        "group": "start",
        "description": "Put arguments before verbs, explicitly stage a local value, then continue it through a direct OSV chain.",
        "file": "functions_osv.rl",
        "kind": "run",
        "expectedOutput": "42\n",
        "source": "// Arguments come before verbs; adjacent verbs continue the value flow.\nfun add: (left: Int32, right: Int32) -> Int32 = {\n    left + right\n}\n\nfun increment: (value: Int32) -> Int32 = {\n    value + 1\n}\n\nfun main: () = {\n    // The semicolon stages a named value before a new identifier-led expression.\n    val total = (20, 21) add;\n    total increment println\n}\n"
    },
    {
        "id": "scopedCollections",
        "title": "Map through a scope",
        "group": "start",
        "description": "Name map's scoped value with a lambda binder, then name both values in a fold scope.",
        "file": "scoped_collections.rl",
        "kind": "run",
        "expectedOutput": "43\n",
        "source": "// A verb can open a typed scope; the complete clause feeds the next value flow.\nfun main: () = {\n    // A lambda binder names the scoped value without a separate local declaration.\n    val shifted = [20, 21] map { |value|\n        value + 1\n    }\n    ((shifted, 0) fold { |total, value|\n        total + value\n    }) println\n}\n"
    },
    {
        "id": "records",
        "title": "Build a record",
        "group": "start",
        "description": "Construct a record with colon fields and pass it to a function.",
        "file": "records.rl",
        "kind": "run",
        "expectedOutput": "42\n",
        "source": "// Record fields use colons in declarations and literals.\nrecord Point {\n    x: Int32\n    y: Int32\n}\n\nfun read_x: (point: Point) -> Int32 = {\n    point.x\n}\n\nfun main: () = {\n    Point { x: 42, y: 7 } read_x println\n}\n"
    },
    {
        "id": "optionMatch",
        "title": "Match an option",
        "group": "start",
        "description": "Handle both Some and None with an exhaustive match.",
        "file": "option_match.rl",
        "kind": "run",
        "expectedOutput": "42\n",
        "source": "// Built-in options are handled with an exhaustive match.\nfun choose: (value: Option<Int32>) -> Int32 = {\n    value match {\n        Some(number) => { number }\n        None => { 0 }\n    }\n}\n\nfun main: () = {\n    42 Option::Some choose println\n}\n"
    },
    {
        "id": "resultError",
        "title": "Result with a custom error",
        "group": "current",
        "description": "Carry a domain-specific enum through Result and turn the error into useful output.",
        "file": "result_error.rl",
        "kind": "run",
        "expectedOutput": "invalid code\n",
        "source": "// A closed enum gives Result a domain-specific error type.\nenum DecodeError {\n    Invalid(String)\n}\n\nfun decode: (code: Int32) -> Result<Int32, DecodeError> = {\n    code == 0 then {\n        42 Result::Ok\n    } else {\n        \"invalid code\" DecodeError::Invalid Result::Err\n    }\n}\n\nfun explain: (result: Result<Int32, DecodeError>) -> String = {\n    result match {\n        Ok(value) => { \"valid code\" }\n        Err(error) => {\n            error match {\n                DecodeError::Invalid(message) => { message }\n            }\n        }\n    }\n}\n\nfun main: () = {\n    1 decode explain println\n}\n"
    },
    {
        "id": "formContract",
        "title": "Generic form contract",
        "group": "current",
        "description": "Follow a form from its contract, through a concrete adoption, into an explicit generic bound.",
        "file": "form_contract.rl",
        "kind": "run",
        "expectedOutput": "ready\n",
        "source": "// form declares a contract, takes adopts it, and of requires it.\nform Labelled {\n    fun label: (self: Self) -> String\n}\n\nrecord Badge {\n    text: String\n}\n\nBadge takes Labelled {\n    fun label: (self: Badge) -> String = {\n        self.text\n    }\n}\n\nfun render: <T of Labelled>(value: T) -> String = {\n    value label\n}\n\nfun main: () = {\n    Badge { text: \"ready\" } render println\n}\n"
    },
    {
        "id": "displayTypes",
        "title": "Display across types",
        "group": "current",
        "description": "Use one output function for prelude types and a record that explicitly adopts Display.",
        "file": "display_types.rl",
        "kind": "run",
        "expectedOutput": "42\nbuilt-in String\nrecord adoption\n",
        "source": "// println accepts any type with a Display adoption.\nrecord Notice {\n    text: String\n}\n\nNotice takes Display {\n    fun display: (self: Notice) -> String = {\n        self.text\n    }\n}\n\nfun main: () = {\n    42 println\n    \"built-in String\" println\n    Notice { text: \"record adoption\" } println\n}\n"
    },
    {
        "id": "affineDiagnostic",
        "title": "Use-after-consume error",
        "group": "diagnostics",
        "description": "See the compiler reject a second use of an affine String binding.",
        "file": "affine_diagnostic.rl",
        "kind": "diagnostic",
        "expectedDiagnostic": "affine type violation",
        "source": "// A non-Copy binding can be consumed at most once.\nfun consume: (value: String) -> () = {\n    value println\n}\n\nfun main: () = {\n    val message = \"use me once\";\n    message consume;\n    message consume\n}\n"
    }
];

export const examplesById = Object.fromEntries(examples.map((example) => [example.id, example]));
