```restrict
value match {
    Some(x) => {
        x > 0 then {
            x |> process
        } else {
            x |> handle_negative
        }
    }
    None => { () default_value }
}
```
