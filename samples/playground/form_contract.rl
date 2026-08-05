// form declares a contract, takes adopts it, and of requires it.
form Labelled {
    fun label: (self: Self) -> String
}

record Badge {
    text: String
}

Badge takes Labelled {
    fun label: (self: Badge) -> String = {
        self.text
    }
}

fun render: <T of Labelled>(value: T) -> String = {
    value |> label
}

fun main: () = {
    Badge { text: "ready" } |> render |> println
}
