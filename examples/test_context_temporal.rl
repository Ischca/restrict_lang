// Experimental design sketch: TAT is outside the default v0.0.1 release gate,
// so this file is not a runnable v0.0.1 release example.

context FileSystem<~fs> {
    open: (String, (File<~fs>) -> Unit) -> Unit
}
