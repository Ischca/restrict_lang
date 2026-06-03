// Spread Destructuring Design Note - Restrict Language
//
// This file is intentionally comment-only. It is visible as a non-release
// example, but it is not runnable v0.0.1 source.
//
// v0.0.1 public source should use current Restrict syntax:
// - OSV calls such as value |> normalize_profile or (left, right) merge_pair
// - val bindings, not let
// - record fields and record literals with colon field syntax
// - full record reconstruction when changing a value
//
// Current v0.0.1 shape for an explicit update:
//
//   record UserProfile {
//       name: String,
//       age: Int32,
//       email: String
//   }
//
//   fun birthday_profile: (profile: UserProfile) -> UserProfile = {
//       val UserProfile { name, age, email } = profile;
//
//       UserProfile {
//           name: name,
//           age: age + 1,
//           email: email
//       }
//   }
//
//   profile |> birthday_profile
//
// Future design sketch only:
//
//   A later spread-pattern pass may allow extracting selected fields while
//   keeping the remaining fields in a rest binding:
//
//       UserProfile { name, ...rest }
//
//   A later record-update pass may allow differential updates from a base
//   record:
//
//       UserProfile {
//           ...profile,
//           age: 31
//       }
//
// Those spread forms are intentionally not presented as executable source in
// this file. They need parser, type-checker, and codegen support before they
// can become public runnable examples.
