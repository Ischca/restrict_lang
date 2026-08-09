pub fun run_consume: (value: Int32) -> () = {
    value |> wit_import_consume_8e58149df9a80a9dbe091ff3
}

pub fun run_long: (value: Int64) -> Int64 = {
    value |> wit_import_echo_long_19ee87defab63e8e56311bdb
}

pub fun run_marker: (value: Char) -> Char = {
    value |> wit_import_echo_marker_871327b8e191224f52fe5386
}

pub fun run_ratio: (value: Float64) -> Float64 = {
    value |> wit_import_echo_ratio_5360b31f2a1ffcadc1fdea71
}

pub fun run_s32: (value: Int32) -> Int32 = {
    value |> wit_import_increment_36267bdf9047e8973de0cb9e
}

pub fun run_bool: (value: Boolean) -> Boolean = {
    value |> wit_import_invert_859ba0bad8b54c38586b2606
}
