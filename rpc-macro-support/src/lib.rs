use jsonrpc_core::{Error as RpcError, Params, Result as RpcResult, Value};
use log::debug;
use serde::de::DeserializeOwned;
use serde_json::from_value;

pub fn parse_params<D: DeserializeOwned + std::fmt::Debug + 'static>(
    param: Params,
) -> RpcResult<D> {
    debug!("In-> {:?}", param);
    let value = match param {
        Params::None => Value::Array(Vec::new()),
        Params::Array(mut vec) => match vec.len() {
            0 => Value::Array(Vec::new()),
            1 => vec.remove(0),
            _ => Value::Array(vec),
        },
        Params::Map(map) => Value::Object(map),
    };

    debug!("Thru-> {:?}", value);
    from_value::<D>(value)
        .map(|out| {
            debug!("Out-> {:?}", out);
            out
        })
        .map_err(|err| {
            debug!("Err-> expected {}", err);
            RpcError::invalid_params(format!("expected {}", err))
        })
}
