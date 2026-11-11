//! JSON-RPC 2.0 over lines: one request per line, one response per line. Parameters are positional.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
/// The call was understood but could not be carried out (for example removing a locked session).
pub const REFUSED: i32 = -32000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} (code {})", self.message, self.code)
    }
}

impl std::error::Error for RpcError {}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// `None` for a notification, which gets no reply.
    pub id: Option<Value>,
    pub method: String,
    pub params: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct RawRequest {
    jsonrpc: Option<String>,
    id: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

/// Parses one request line.
pub fn parse_request(line: &str) -> Result<Request, RpcError> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| RpcError::new(PARSE_ERROR, format!("not valid JSON: {error}")))?;
    let raw: RawRequest = serde_json::from_value(value)
        .map_err(|_| RpcError::new(INVALID_REQUEST, "the request is not a JSON object"))?;
    if raw.jsonrpc.as_deref() != Some("2.0") {
        return Err(RpcError::new(INVALID_REQUEST, "\"jsonrpc\" must be \"2.0\""));
    }
    let method = raw
        .method
        .filter(|method| !method.is_empty())
        .ok_or_else(|| RpcError::new(INVALID_REQUEST, "\"method\" is missing"))?;
    let params = match raw.params {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items,
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "\"params\" must be an array")),
    };
    Ok(Request { id: raw.id, method, params })
}

/// A response line (no trailing newline).
pub fn success_line(id: &Value, result: &Value) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

pub fn error_line(id: &Value, error: &RpcError) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": error.code, "message": error.message } })
        .to_string()
}

/// A request line for `method` with `params`.
pub fn request_line(id: u64, method: &str, params: &[Value]) -> String {
    serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

/// The outcome of a call as read from a response line.
pub fn parse_response(line: &str) -> Result<Value, RpcError> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| RpcError::new(PARSE_ERROR, format!("bad response: {error}")))?;
    if let Some(error) = value.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(i64::from(REFUSED)) as i32;
        let message = error.get("message").and_then(Value::as_str).unwrap_or("unknown error");
        return Err(RpcError::new(code, message));
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn given_a_request_line_when_parsing_then_method_params_and_id_are_read() {
        let request =
            parse_request(r#"{"jsonrpc":"2.0","id":7,"method":"splitSessionLeftRight","params":[3]}"#).expect("parses");
        assert_eq!(
            request,
            Request { id: Some(json!(7)), method: "splitSessionLeftRight".into(), params: vec![json!(3)] }
        );
    }

    #[test]
    fn given_no_params_or_null_when_parsing_then_the_list_is_empty() {
        assert!(parse_request(r#"{"jsonrpc":"2.0","id":1,"method":"addSession"}"#).expect("parses").params.is_empty());
        assert!(
            parse_request(r#"{"jsonrpc":"2.0","id":1,"method":"addSession","params":null}"#)
                .expect("parses")
                .params
                .is_empty()
        );
    }

    #[test]
    fn given_a_notification_when_parsing_then_there_is_no_id() {
        assert_eq!(parse_request(r#"{"jsonrpc":"2.0","method":"toggleWindowState"}"#).expect("parses").id, None);
    }

    #[test]
    fn given_broken_requests_when_parsing_then_the_error_code_says_what_is_wrong() {
        assert_eq!(parse_request("{not json").unwrap_err().code, PARSE_ERROR);
        assert_eq!(parse_request("[1,2]").unwrap_err().code, INVALID_REQUEST);
        assert_eq!(parse_request(r#"{"id":1,"method":"x"}"#).unwrap_err().code, INVALID_REQUEST);
        assert_eq!(parse_request(r#"{"jsonrpc":"1.0","id":1,"method":"x"}"#).unwrap_err().code, INVALID_REQUEST);
        assert_eq!(parse_request(r#"{"jsonrpc":"2.0","id":1}"#).unwrap_err().code, INVALID_REQUEST);
        assert_eq!(parse_request(r#"{"jsonrpc":"2.0","id":1,"method":""}"#).unwrap_err().code, INVALID_REQUEST);
        assert_eq!(
            parse_request(r#"{"jsonrpc":"2.0","id":1,"method":"x","params":{"a":1}}"#).unwrap_err().code,
            INVALID_PARAMS
        );
    }

    #[test]
    fn given_a_result_when_encoding_then_a_response_line_carries_the_id_and_result() {
        let line = success_line(&json!(4), &json!("0,1"));
        assert_eq!(serde_json::from_str::<Value>(&line).expect("json"), json!({"jsonrpc":"2.0","id":4,"result":"0,1"}));
        assert!(!line.contains('\n'));
    }

    #[test]
    fn given_an_error_when_encoding_then_the_code_and_message_are_present() {
        let line = error_line(&json!(null), &RpcError::new(METHOD_NOT_FOUND, "no such method"));
        let value: Value = serde_json::from_str(&line).expect("json");
        assert_eq!(value["error"]["code"], json!(METHOD_NOT_FOUND));
        assert_eq!(value["error"]["message"], json!("no such method"));
    }

    #[test]
    fn given_a_call_when_building_the_request_line_then_the_server_can_parse_it() {
        let line = request_line(2, "setTabTitle", &[json!(0), json!("build")]);
        let request = parse_request(&line).expect("parses");
        assert_eq!((request.method.as_str(), request.params.len()), ("setTabTitle", 2));
    }

    #[test]
    fn given_response_lines_when_reading_then_results_and_errors_come_apart() {
        assert_eq!(parse_response(r#"{"jsonrpc":"2.0","id":1,"result":5}"#), Ok(json!(5)));
        assert_eq!(parse_response(r#"{"jsonrpc":"2.0","id":1}"#), Ok(Value::Null));
        let error = parse_response(r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"no"}}"#).unwrap_err();
        assert_eq!((error.code, error.message.as_str()), (METHOD_NOT_FOUND, "no"));
        assert_eq!(parse_response("garbage").unwrap_err().code, PARSE_ERROR);
        assert!(error.to_string().contains("-32601"));
    }
}
