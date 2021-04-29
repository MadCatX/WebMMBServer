use serde_json;

use crate::server::api;

fn value_to_string(value: serde_json::Value) -> Result<String, serde_json::Error> {
    match serde_json::from_value::<i32>(value.clone()) {
        Ok(v) => return Ok(i32::to_string(&v)),
        Err(_) => (),
    }

    match serde_json::from_value::<f64>(value.clone()) {
        Ok(v) => return Ok(f64::to_string(&v).replace(",", ".")),
        Err(_) => (),
    }

    match serde_json::from_value::<bool>(value.clone()) {
        Ok(v) => match v {
            true => return Ok(String::from("True")),
            false => return Ok(String::from("False")),
        }
        Err(_) => (),
    }

    serde_json::from_value::<String>(value)
}

pub fn to_txt(params: &api::JsonAdvancedParameters) -> Result<String, serde_json::Error> {
    let mut txt = String::new();
    for (k, v) in params.iter() {
        match value_to_string(v.clone()) {
            Ok(s) => txt.push_str(format!("{} {}\n", k, s).as_str()),
            Err(e) => return Err(e),
        }
    }
    Ok(txt)
}
