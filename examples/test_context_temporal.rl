// ⚠ NOT YET COMPILABLE — requires: context declarations
// Planned for: v0.2.0+ (Temporal Affine Types)
//
context FileSystem<~fs> {
    open: (String, (File<~fs>) -> Unit) -> Unit
}