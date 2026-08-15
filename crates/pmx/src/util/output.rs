use serde::Serialize;
use serde_json::{Value, json};

pub fn ok(message: impl Into<String>, data: Value) -> Value {
    json!({
        "status": "ok",
        "message": message.into(),
        "data": data,
    })
}

pub fn print_json_value(value: &Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serialize JSON output")
    );
}

pub fn to_value<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value)
        .unwrap_or_else(|err| json!({ "serialization_error": err.to_string() }))
}
