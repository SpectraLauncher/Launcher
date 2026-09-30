use extism_pdk::*;
use serde::Deserialize;
use serde_json::{json, Value};

#[host_fn]
extern "ExtismHost" {
    fn spectra_call(request: String) -> String;
}

fn spectra(method: &str, params: Value) -> Result<Value, Error> {
    let request = json!({ "method": method, "params": params }).to_string();
    let reply: Value = serde_json::from_str(&unsafe { spectra_call(request)? })?;
    match reply.get("error") {
        Some(error) => Err(Error::msg(error.to_string())),
        None => Ok(reply.get("result").cloned().unwrap_or(Value::Null)),
    }
}

#[derive(Deserialize)]
struct Numbers {
    values: Vec<f64>,
}

#[plugin_fn]
pub fn stats(Json(input): Json<Numbers>) -> FnResult<Json<Value>> {
    let count = input.values.len();
    let sum: f64 = input.values.iter().sum();
    let max = input.values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(Json(json!({
        "count": count,
        "sum": sum,
        "mean": if count == 0 { 0.0 } else { sum / count as f64 },
        "max": if count == 0 { Value::Null } else { json!(max) },
    })))
}

#[plugin_fn]
pub fn instance_count(_input: String) -> FnResult<Json<Value>> {
    let instances = spectra("instances.list", json!({}))?;
    Ok(Json(json!({ "instances": instances.as_array().map_or(0, Vec::len) })))
}

#[plugin_fn]
pub fn forbidden(_input: String) -> FnResult<Json<Value>> {
    let outcome = spectra("logs.list", json!({ "id": "any" }));
    Ok(Json(json!({
        "refused": outcome.is_err(),
        "detail": outcome.err().map(|e| e.to_string()),
    })))
}

#[plugin_fn]
pub fn remember(input: String) -> FnResult<Json<Value>> {
    spectra("storage.set", json!({ "key": "backend", "value": input }))?;
    Ok(Json(spectra("storage.get", json!({ "key": "backend" }))?))
}

#[plugin_fn]
pub fn spin(_input: String) -> FnResult<String> {
    loop {
        std::hint::spin_loop();
    }
}
