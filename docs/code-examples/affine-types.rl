// Affine type examples
record Ticket {
    severity: Int32,
    owner: String
}

fun consume_message: (message: String) -> () = {
    message |> println
}

fun affine_example: () -> () = {
    val message = "This can only be used once"

    message |> consume_message

    // message |> consume_message  // Error: message already consumed
}

// Record updates use postfix .clone syntax.
fun lower_severity: (ticket: Ticket) -> Ticket = {
    ticket.clone {
        severity: 1,
        owner: "ops"
    }
}

fun clone_example: () -> Int32 = {
    val ticket = Ticket { severity: 5, owner: "ops" }
    val lowered = ticket |> lower_severity

    lowered.severity
}
