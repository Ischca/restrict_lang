// Field Access Patterns Design Note - Restrict Language
//
// This file is intentionally comment-only. It is visible as a non-release
// example, but it is not runnable v0.0.1 source.
//
// v0.0.1 public source should keep field access examples conservative:
// - use OSV calls for helper functions
// - bind with val, not let
// - write record fields and record literals with colon field syntax
// - destructure a record once when multiple fields are needed
//
// Current v0.0.1 shape for reading several fields:
//
//   record Point2D {
//       x: Float64,
//       y: Float64
//   }
//
//   fun squared_distance_from_origin: (point: Point2D) -> Float64 = {
//       val Point2D { x, y } = point;
//
//       x * x + y * y
//   }
//
//   point |> squared_distance_from_origin
//
// Current v0.0.1 shape for moving several affine fields together:
//
//   record User {
//       id: Int32,
//       name: String,
//       email: String
//   }
//
//   fun user_label: (user: User) -> String = {
//       val User { id, name, email } = user;
//
//       name
//   }
//
// Future design sketch only:
//
//   More precise copyable-field access, view prototypes, and method-like field
//   helpers may become useful later. Until those rules are implemented and
//   tested, examples should avoid active source that depends on repeated
//   reads from the same affine record, object-style method calls, impl blocks,
//   frozen record declarations, or resource handles that are not part of the
//   v0.0.1 public surface.
