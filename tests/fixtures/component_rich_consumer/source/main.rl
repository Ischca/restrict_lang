pub record Packet {
    label: String,
    values: List<Int32>,
    fixed: Array<Int32, 3>,
    maybe: Option<String>,
    outcome: Result<List<Int32>, String>,
    status: Status,
    event: Event
}

pub enum Status {
    Ready
    Paused
}

pub enum Event {
    Idle
    Message(String)
    Values(List<Int32>)
}

pub fun echo_string: (value: String) -> String = {
    value
}

pub fun echo_list: (value: List<Int32>) -> List<Int32> = {
    value
}

pub fun echo_array: (value: Array<Int32, 3>) -> Array<Int32, 3> = {
    value
}

pub fun echo_packet: (value: Packet) -> Packet = {
    value
}

pub fun echo_option: (value: Option<String>) -> Option<String> = {
    value
}

pub fun echo_result: (value: Result<List<Int32>, String>) -> Result<List<Int32>, String> = {
    value
}

pub fun echo_status: (value: Status) -> Status = {
    value
}

pub fun echo_event: (value: Event) -> Event = {
    value
}
