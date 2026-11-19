// Practical log analyzer showcase combining stdlib modules, algebraic enums,
// nested pattern matching, affine-safe record updates, ranges, loops, and maps.

import std.string.*
import std.list.*
import std.result.*
import std.option.*
import std.map.*

enum LogLevel {
    Info,
    Warn,
    Error
}

LogLevel takes Display {
    fun display: (self: LogLevel) -> String = {
        self match {
            LogLevel::Info => { "INFO" }
            LogLevel::Warn => { "WARN" }
            LogLevel::Error => { "ERROR" }
        }
    }
}

record LogEntry {
    level: LogLevel,
    service: String,
    latency_ms: Int32,
    escalated: Boolean
}

fun parse_level: (raw: String) -> Result<LogLevel, String> = {
    mut val clean = raw |> string_trim
    (clean == "INFO") then {
        (() LogLevel::Info) Result::Ok
    } else (clean == "WARN") then {
        (() LogLevel::Warn) Result::Ok
    } else (clean == "ERROR") then {
        (() LogLevel::Error) Result::Ok
    } else {
        ("unknown level: " + clean) Result::Err
    }
}

fun extract_kv_value: (token: String, prefix: String) -> Option<String> = {
    val parts = (token, "=") string_split
    parts match {
        [key, value] => {
            ((key |> string_trim) == prefix) then {
                (value |> string_trim) Option::Some
            } else {
                () Option::None
            }
        }
        _ => { () Option::None }
    }
}

fun parse_latency_ms: (raw_ms: String) -> Result<Int32, String> = {
    val parsed = raw_ms |> string_trim |> string_parse_int
    (parsed, "invalid integer") option_ok_or result_and_then { |ms|
        (ms < 0) then {
            ("negative latency") Result::Err
        } else {
            (ms) Result::Ok
        }
    }
}

fun parse_log_line: (line: String) -> Result<LogEntry, String> = {
    mut val trimmed = line |> string_trim
    (trimmed |> string_is_empty) then {
        return ("empty line") Result::Err
    }

    val tokens = trimmed |> string_words
    tokens match {
        [lvl_tok, svc_tok, ms_tok] => {
            val lvl_opt = (lvl_tok, "level") extract_kv_value
            val svc_opt = (svc_tok, "service") extract_kv_value
            val ms_opt = (ms_tok, "ms") extract_kv_value

            val lvl_res = (lvl_opt, "missing level") option_ok_or result_and_then { |raw|
                raw |> parse_level
            }
            val svc_res = (svc_opt, "missing service") option_ok_or
            val ms_res = (ms_opt, "missing ms") option_ok_or result_and_then { |raw|
                raw |> parse_latency_ms
            }

            lvl_res match {
                Ok (level) => {
                    svc_res match {
                        Ok (service) => {
                            ms_res match {
                                Ok (latency_ms) => {
                                    val base = LogEntry {
                                        level: level,
                                        service: service,
                                        latency_ms: latency_ms,
                                        escalated: false
                                    }
                                    // Payload-free enum == and Copy semantics + consumed-val .clone update
                                    val needs_escalation = (level == () LogLevel::Error) || ((level == () LogLevel::Warn) && (latency_ms >= 200))
                                    val final_entry = needs_escalation then {
                                        base.clone { escalated: true }
                                    } else {
                                        base
                                    }
                                    (final_entry) Result::Ok
                                }
                                Err (err) => { (err) Result::Err }
                            }
                        }
                        Err (err) => { (err) Result::Err }
                    }
                }
                Err (err) => { (err) Result::Err }
            }
        }
        _ => { ("malformed record") Result::Err }
    }
}

fun bump_service_count: (counts: StringMap<Int32>, service: String) -> StringMap<Int32> = {
    mut val map_ref = counts
    mut val svc = service
    val prev = (map_ref, svc) smap_get option_unwrap_or_else { || 0 }
    (map_ref, svc, prev + 1) smap_put
}

fun analyze_batch: (raw_log: String, max_lines: Int32) -> Int32 = {
    mut val lines = raw_log |> string_lines
    val total_lines = lines |> list_length
    mut val counts: StringMap<Int32> = () smap_empty
    mut val entries: List<LogEntry> = []
    mut val idx = 0

    (idx < total_lines) while {
        (idx >= max_lines) then {
            break
        }
        mut val line = (lines, idx) list_get
        idx = idx + 1
        ((line |> string_trim) |> string_is_empty) then {
            continue
        }
        (line |> parse_log_line) match {
            Ok (entry) => {
                val LogEntry { level, service, latency_ms, escalated } = entry
                mut val svc = service
                counts = (counts, svc) bump_service_count
                val updated = LogEntry {
                    level: level,
                    service: svc,
                    latency_ms: latency_ms,
                    escalated: escalated
                }
                entries = (entries, updated) list_append
            }
            Err (_) => {
                continue
            }
        }
    }

    val entry_count = entries |> list_length
    val range_bonus = [0..entry_count]
    |> map { it + 1 }
    |> { (it, 0) |> fold { |acc, x| acc + x } }
    val has_slow = (entries, | e | e.latency_ms >= 150) list_any
    val slow_bonus = has_slow then { 100 } else { 0 }
    val auth_hits = (counts, "auth")
    |> smap_get
    ?? 0
    val service_kinds = counts |> smap_len

    entry_count * 1000 + service_kinds * 100 + auth_hits * 10 + range_bonus + slow_bonus
}

export fun analyze_logs_summary: () -> Int32 = {
    val sample = "level=INFO service=edge ms=18\n\nlevel=ERROR service=auth ms=142\nlevel=WARN service=auth ms=260\nlevel=BAD service=skip ms=10\nlevel=INFO service=billing ms=95\n"
    (sample, 10) analyze_batch
}

fun main: () -> () = {
    val err_level = () LogLevel::Error
    val summary = () analyze_logs_summary
    ("level=" + (err_level display) + " summary=" + (summary display)) |> println
}
