context FileSystem<~fs> {
    open: (String, (File<~fs>) -> Unit) -> Unit
}