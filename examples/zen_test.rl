// v0.0.1 minimal-flow smoke example.
// Replaces experimental symbolic syntax with current Restrict forms.

record Flow {
    input: Int32,
    output: Int32
}

fun square: (value: Int32) -> Int32 = {
    value * value
}

fun flow_score: (flow: Flow) -> Int32 = {
    val Flow { input, output } = flow;
    output >= input then {
        output - input
    } else {
        0
    }
}

fun main: () -> Int32 = {
    val value = 6;
    val output = value |> square;
    val flow = Flow { input: value, output: output };

    flow |> flow_score
}
