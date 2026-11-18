// Standard Library: Map
//
// Pure-Restrict associative containers `StringMap<V>` and `IntMap<V>`
// implemented with parallel key/value lists for deterministic, GC-free,
// affine-safe execution.

pub record StringMap<V> {
    keys: List<String>,
    values: List<V>
}

pub record IntMap<V> {
    keys: List<Int32>,
    values: List<V>
}

pub fun smap_empty: <V>() -> StringMap<V> = {
    StringMap { keys: [], values: [] }
}

pub fun smap_len: <V>(m: StringMap<V>) -> Int32 = {
    m match {
        StringMap { keys, values } => { keys |> list_length }
    }
}

pub fun smap_keys: <V>(m: StringMap<V>) -> List<String> = {
    m match {
        StringMap { keys, values } => { keys }
    }
}

pub fun smap_values: <V>(m: StringMap<V>) -> List<V> = {
    m match {
        StringMap { keys, values } => { values }
    }
}

pub fun smap_contains: <V>(m: StringMap<V>, key: String) -> Boolean = {
    m match {
        StringMap { keys, values } => {
            mut val ks = keys
            mut val target = key
            val len = ks |> list_length
            mut val i = 0
            mut val found = false
            (i < len) && (found |> not) while {
                (((ks, i) list_get) == target) then {
                    found = true
                } else {
                    i = i + 1
                }
            }
            found
        }
    }
}

pub fun smap_get: <V>(m: StringMap<V>, key: String) -> Option<V> = {
    m match {
        StringMap { keys, values } => {
            mut val ks = keys
            mut val vs = values
            mut val target = key
            val len = ks |> list_length
            mut val i = 0
            mut val found_idx = -1
            (i < len) && (found_idx < 0) while {
                (((ks, i) list_get) == target) then {
                    found_idx = i
                } else {
                    i = i + 1
                }
            }
            found_idx >= 0 then {
                ((vs, found_idx) list_get) Option::Some
            } else {
                () Option::None
            }
        }
    }
}

pub fun smap_put: <V>(m: StringMap<V>, key: String, value: V) -> StringMap<V> = {
    m match {
        StringMap { keys, values } => {
            mut val ks = keys
            mut val vs = values
            mut val target = key
            val len = ks |> list_length
            mut val i = 0
            mut val found_idx = -1
            (i < len) && (found_idx < 0) while {
                (((ks, i) list_get) == target) then {
                    found_idx = i
                } else {
                    i = i + 1
                }
            }
            found_idx < 0 then {
                StringMap {
                    keys: (ks, target) list_append,
                    values: (vs, value) list_append
                }
            } else {
                found_idx == 0 then {
                    mut val new_vs = [value]
                    mut val j = 1
                    j < len while {
                        new_vs = (new_vs, (vs, j) list_get) list_append
                        j = j + 1
                    }
                    StringMap { keys: ks, values: new_vs }
                } else {
                    mut val new_vs = [(vs, 0) list_get]
                    mut val j = 1
                    j < found_idx while {
                        new_vs = (new_vs, (vs, j) list_get) list_append
                        j = j + 1
                    }
                    new_vs = (new_vs, value) list_append
                    mut val k = found_idx + 1
                    k < len while {
                        new_vs = (new_vs, (vs, k) list_get) list_append
                        k = k + 1
                    }
                    StringMap { keys: ks, values: new_vs }
                }
            }
        }
    }
}

pub fun smap_remove: <V>(m: StringMap<V>, key: String) -> StringMap<V> = {
    m match {
        StringMap { keys, values } => {
            mut val ks = keys
            mut val vs = values
            mut val target = key
            val len = ks |> list_length
            mut val i = 0
            mut val found_idx = -1
            (i < len) && (found_idx < 0) while {
                (((ks, i) list_get) == target) then {
                    found_idx = i
                } else {
                    i = i + 1
                }
            }
            found_idx < 0 then {
                StringMap { keys: ks, values: vs }
            } else {
                len <= 1 then {
                    StringMap { keys: [], values: [] }
                } else {
                    val first_i = found_idx == 0 then { 1 } else { 0 }
                    mut val new_ks = [(ks, first_i) list_get]
                    mut val new_vs = [(vs, first_i) list_get]
                    mut val j = first_i + 1
                    j < len while {
                        j != found_idx then {
                            new_ks = (new_ks, (ks, j) list_get) list_append
                            new_vs = (new_vs, (vs, j) list_get) list_append
                        }
                        j = j + 1
                    }
                    StringMap { keys: new_ks, values: new_vs }
                }
            }
        }
    }
}

pub fun imap_empty: <V>() -> IntMap<V> = {
    IntMap { keys: [], values: [] }
}

pub fun imap_len: <V>(m: IntMap<V>) -> Int32 = {
    m match {
        IntMap { keys, values } => { keys |> list_length }
    }
}

pub fun imap_keys: <V>(m: IntMap<V>) -> List<Int32> = {
    m match {
        IntMap { keys, values } => { keys }
    }
}

pub fun imap_values: <V>(m: IntMap<V>) -> List<V> = {
    m match {
        IntMap { keys, values } => { values }
    }
}

pub fun imap_contains: <V>(m: IntMap<V>, key: Int32) -> Boolean = {
    m match {
        IntMap { keys, values } => {
            mut val ks = keys
            val len = ks |> list_length
            mut val i = 0
            mut val found = false
            (i < len) && (found |> not) while {
                (((ks, i) list_get) == key) then {
                    found = true
                } else {
                    i = i + 1
                }
            }
            found
        }
    }
}

pub fun imap_get: <V>(m: IntMap<V>, key: Int32) -> Option<V> = {
    m match {
        IntMap { keys, values } => {
            mut val ks = keys
            mut val vs = values
            val len = ks |> list_length
            mut val i = 0
            mut val found_idx = -1
            (i < len) && (found_idx < 0) while {
                (((ks, i) list_get) == key) then {
                    found_idx = i
                } else {
                    i = i + 1
                }
            }
            found_idx >= 0 then {
                ((vs, found_idx) list_get) Option::Some
            } else {
                () Option::None
            }
        }
    }
}

pub fun imap_put: <V>(m: IntMap<V>, key: Int32, value: V) -> IntMap<V> = {
    m match {
        IntMap { keys, values } => {
            mut val ks = keys
            mut val vs = values
            val len = ks |> list_length
            mut val i = 0
            mut val found_idx = -1
            (i < len) && (found_idx < 0) while {
                (((ks, i) list_get) == key) then {
                    found_idx = i
                } else {
                    i = i + 1
                }
            }
            found_idx < 0 then {
                IntMap {
                    keys: (ks, key) list_append,
                    values: (vs, value) list_append
                }
            } else {
                found_idx == 0 then {
                    mut val new_vs = [value]
                    mut val j = 1
                    j < len while {
                        new_vs = (new_vs, (vs, j) list_get) list_append
                        j = j + 1
                    }
                    IntMap { keys: ks, values: new_vs }
                } else {
                    mut val new_vs = [(vs, 0) list_get]
                    mut val j = 1
                    j < found_idx while {
                        new_vs = (new_vs, (vs, j) list_get) list_append
                        j = j + 1
                    }
                    new_vs = (new_vs, value) list_append
                    mut val k = found_idx + 1
                    k < len while {
                        new_vs = (new_vs, (vs, k) list_get) list_append
                        k = k + 1
                    }
                    IntMap { keys: ks, values: new_vs }
                }
            }
        }
    }
}

pub fun imap_remove: <V>(m: IntMap<V>, key: Int32) -> IntMap<V> = {
    m match {
        IntMap { keys, values } => {
            mut val ks = keys
            mut val vs = values
            val len = ks |> list_length
            mut val i = 0
            mut val found_idx = -1
            (i < len) && (found_idx < 0) while {
                (((ks, i) list_get) == key) then {
                    found_idx = i
                } else {
                    i = i + 1
                }
            }
            found_idx < 0 then {
                IntMap { keys: ks, values: vs }
            } else {
                len <= 1 then {
                    IntMap { keys: [], values: [] }
                } else {
                    val first_i = found_idx == 0 then { 1 } else { 0 }
                    mut val new_ks = [(ks, first_i) list_get]
                    mut val new_vs = [(vs, first_i) list_get]
                    mut val j = first_i + 1
                    j < len while {
                        j != found_idx then {
                            new_ks = (new_ks, (ks, j) list_get) list_append
                            new_vs = (new_vs, (vs, j) list_get) list_append
                        }
                        j = j + 1
                    }
                    IntMap { keys: new_ks, values: new_vs }
                }
            }
        }
    }
}
