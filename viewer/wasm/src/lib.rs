//! Browser boundary: the native replay verifier runs unchanged in a Worker.
use replay_export::{
    data::{ExportOptions, build_payload},
    json::write_json,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_replay(bytes: &[u8]) -> Result<String, JsValue> {
    let options = ExportOptions {
        dev: true,
        faction: None,
        meta: None,
    };
    let (payload, _) = build_payload(bytes, &options).map_err(|e| JsValue::from_str(&e))?;
    write_json(&payload).map_err(|e| JsValue::from_str(&e.to_string()))
}
