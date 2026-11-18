// Standard Library: String
//
// Pure-Restrict string helpers built on compiler-registered string primitives
// (`string_length`, `string_byte_at`, `string_slice`, `string_index_of`,
// `string_split`, `string_join`, `string_ends_with`, `char_from_code`).

pub fun string_is_empty: (s: String) -> Boolean = {
    (s |> string_length) == 0
}

pub fun string_non_empty: (s: String) -> Boolean = {
    (s |> string_length) > 0
}

pub fun string_repeat: (s: String, count: Int32) -> String = {
    count <= 0 then {
        ""
    } else {
        mut val chunk = s;
        mut val parts: List<String> = [];
        mut val i = 0;
        i < count while {
            parts = (parts, chunk) list_append;
            i = i + 1
        };
        (parts, "") string_join
    }
}

pub fun string_join_simple: (parts: List<String>, sep: String) -> String = {
    (parts, sep) string_join
}

pub fun string_lines: (s: String) -> List<String> = {
    mut val text = s;
    val len = text |> string_length;
    len == 0 then {
        []
    } else {
        val trimmed_end = (text, "\n") string_ends_with then {
            ((text, 0, len - 1) string_slice, text) option_unwrap_or
        } else {
            text
        };
        val raw = (trimmed_end, "\n") string_split;
        (raw, |line| {
            mut val current = line;
            val l = current |> string_length;
            (current, "\r") string_ends_with then {
                ((current, 0, l - 1) string_slice, current) option_unwrap_or
            } else {
                current
            }
        }) map
    }
}

pub fun string_words: (s: String) -> List<String> = {
    mut val text = s;
    val len = text |> string_length;
    mut val words: List<String> = [];
    mut val i = 0;
    i < len while {
        mut val skipping = true;
        (i < len) && skipping while {
            val b = ((text, i) string_byte_at, -1) option_unwrap_or;
            ((b == 32) || (b == 9) || (b == 10) || (b == 13)) then {
                i = i + 1
            } else {
                skipping = false
            }
        };
        i < len then {
            val start = i;
            mut val scanning = true;
            (i < len) && scanning while {
                val b = ((text, i) string_byte_at, -1) option_unwrap_or;
                ((b == 32) || (b == 9) || (b == 10) || (b == 13)) then {
                    scanning = false
                } else {
                    i = i + 1
                }
            };
            val word = ((text, start, i) string_slice, "") option_unwrap_or;
            words = (words, word) list_append
        }
    };
    words
}

pub fun string_replace: (s: String, target: String, replacement: String) -> String = {
    mut val source = s;
    mut val needle = target;
    (needle |> string_is_empty) then {
        source
    } else {
        ((source, needle) string_split, replacement) string_join
    }
}

pub fun string_pad_left: (s: String, width: Int32, pad: String) -> String = {
    mut val text = s;
    mut val pad_str = pad;
    val len = text |> string_length;
    val pad_len = pad_str |> string_length;
    (len >= width) || (pad_len <= 0) then {
        text
    } else {
        val needed = width - len;
        val reps = (needed + pad_len - 1) / pad_len;
        mut val full_pad = (pad_str, reps) string_repeat;
        val prefix = ((full_pad, 0, needed) string_slice, full_pad) option_unwrap_or;
        prefix + text
    }
}

pub fun string_pad_right: (s: String, width: Int32, pad: String) -> String = {
    mut val text = s;
    mut val pad_str = pad;
    val len = text |> string_length;
    val pad_len = pad_str |> string_length;
    (len >= width) || (pad_len <= 0) then {
        text
    } else {
        val needed = width - len;
        val reps = (needed + pad_len - 1) / pad_len;
        mut val full_pad = (pad_str, reps) string_repeat;
        val suffix = ((full_pad, 0, needed) string_slice, full_pad) option_unwrap_or;
        text + suffix
    }
}

pub fun string_to_upper_ascii: (s: String) -> String = {
    mut val text = s;
    val len = text |> string_length;
    mut val parts: List<String> = [];
    mut val chunk_start = 0;
    mut val i = 0;
    i < len while {
        val b = ((text, i) string_byte_at, -1) option_unwrap_or;
        ((b >= 97) && (b <= 122)) then {
            i > chunk_start then {
                val chunk = ((text, chunk_start, i) string_slice, "") option_unwrap_or;
                parts = (parts, chunk) list_append
            };
            val ch = ((b - 32) |> char_from_code) match {
                Some(c) => { c |> display }
                None => { "" }
            };
            parts = (parts, ch) list_append;
            chunk_start = i + 1
        };
        i = i + 1
    };
    chunk_start < len then {
        val tail_chunk = ((text, chunk_start, len) string_slice, "") option_unwrap_or;
        parts = (parts, tail_chunk) list_append
    };
    (parts, "") string_join
}

pub fun string_to_lower_ascii: (s: String) -> String = {
    mut val text = s;
    val len = text |> string_length;
    mut val parts: List<String> = [];
    mut val chunk_start = 0;
    mut val i = 0;
    i < len while {
        val b = ((text, i) string_byte_at, -1) option_unwrap_or;
        ((b >= 65) && (b <= 90)) then {
            i > chunk_start then {
                val chunk = ((text, chunk_start, i) string_slice, "") option_unwrap_or;
                parts = (parts, chunk) list_append
            };
            val ch = ((b + 32) |> char_from_code) match {
                Some(c) => { c |> display }
                None => { "" }
            };
            parts = (parts, ch) list_append;
            chunk_start = i + 1
        };
        i = i + 1
    };
    chunk_start < len then {
        val tail_chunk = ((text, chunk_start, len) string_slice, "") option_unwrap_or;
        parts = (parts, tail_chunk) list_append
    };
    (parts, "") string_join
}
