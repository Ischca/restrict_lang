wit_bindgen::generate!({
    path: "../wit",
    world: "producer",
});

struct RustProducer;

use exports::test::external_scalar::calculator::{Event, Packet, State};

impl exports::test::external_scalar::calculator::Guest for RustProducer {
    fn consume(_value: i32) {}

    fn echo_long(value: i64) -> i64 {
        value
    }

    fn echo_marker(value: char) -> char {
        value
    }

    fn echo_ratio(value: f64) -> f64 {
        value
    }

    fn increment(value: i32) -> i32 {
        value + 41
    }

    fn invert(value: bool) -> bool {
        !value
    }

    fn make_string() -> String {
        "Rust producer 🦀界".to_string()
    }

    fn check_string(value: String) -> i32 {
        i32::from(value == "Rust producer 🦀界") * 101
    }

    fn make_list() -> Vec<i32> {
        vec![i32::MIN, 0, i32::MAX]
    }

    fn check_list(value: Vec<i32>) -> i32 {
        i32::from(value == [i32::MIN, 0, i32::MAX]) * 102
    }

    fn make_packet() -> Packet {
        Packet {
            label: "packet 📦".to_string(),
            values: vec![-7, 8, 9],
        }
    }

    fn check_packet(value: Packet) -> i32 {
        i32::from(value.label == "packet 📦" && value.values == [-7, 8, 9]) * 103
    }

    fn make_option() -> Option<String> {
        Some("optional".to_string())
    }

    fn check_option(value: Option<String>) -> i32 {
        i32::from(value.as_deref() == Some("optional")) * 104
    }

    fn make_result() -> Result<String, String> {
        Err("expected-error".to_string())
    }

    fn check_result(value: Result<String, String>) -> i32 {
        i32::from(matches!(value, Err(error) if error == "expected-error")) * 105
    }

    fn make_state() -> State {
        State::Stopped
    }

    fn check_state(value: State) -> i32 {
        i32::from(matches!(value, State::Stopped)) * 106
    }

    fn make_event() -> Event {
        Event::Values(vec![3, 1, 4])
    }

    fn check_event(value: Event) -> i32 {
        i32::from(matches!(value, Event::Values(values) if values == [3, 1, 4])) * 107
    }

    fn make_array() -> Vec<i32> {
        vec![11, 22, 33]
    }

    fn make_bad_array() -> Vec<i32> {
        vec![11, 22]
    }

    fn check_array(value: Vec<i32>) -> i32 {
        i32::from(value == [11, 22, 33]) * 108
    }
}

export!(RustProducer);
