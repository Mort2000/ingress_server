//! Browser boundary: the native replay verifier runs unchanged in a Worker.
use arena_viewer::{data::build_payload, json::write_json};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_replay(bytes: &[u8]) -> Result<String, JsValue> {
    let payload = build_payload(bytes).map_err(|e| JsValue::from_str(&e))?;
    write_json(&payload).map_err(|e| JsValue::from_str(&e.to_string()))
}
