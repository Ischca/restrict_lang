// Legacy standard-library design note.
//
// This file is retained as historical design context, not as the current
// standard library source. The current compiler-registered surface is described
// in LANGUAGE_SPECIFICATION.md and std/.
//
// Current public primitives guaranteed by the specification include:
//
// fun print_string_note: (message: String) -> () = {
//     message |> println
// }
//
// fun print_number_note: (value: Int32) -> () = {
//     value |> print_int
// }
//
// Earlier drafts sketched prototype impl blocks, generic string conversion,
// logical-value helper methods, and a named unit alias here. Those shapes are
// intentionally omitted so this examples directory does not advertise obsolete
// user-facing syntax.
