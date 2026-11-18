// Standard Library: String
//
// Pure-Restrict string helpers built on core string concatenation (`+`) and
// equality (`==`, `!=`).

pub fun string_is_empty: (s: String) -> Boolean = {
    s == ""
}

pub fun string_non_empty: (s: String) -> Boolean = {
    s != ""
}

pub fun string_repeat: (s: String, count: Int32) -> String = {
    mut val chunk = s;
    mut val acc = "";
    mut val i = 0;
    i < count while {
        acc = acc + chunk;
        i = i + 1
    };
    acc
}

pub fun string_pad_left: (s: String, pad: String, repeat_count: Int32) -> String = {
    ((pad, repeat_count) string_repeat) + s
}

pub fun string_join_simple: (parts: List<String>, sep: String) -> String = {
    mut val xs = parts;
    val len = xs |> list_length;
    len == 0 then {
        ""
    } else {
        mut val delimiter = sep;
        mut val acc = (xs, 0) list_get;
        mut val i = 1;
        i < len while {
            acc = (acc + delimiter) + ((xs, i) list_get);
            i = i + 1
        };
        acc
    }
}
