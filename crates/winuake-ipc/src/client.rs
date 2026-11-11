//! The client side: send one call, read one reply, over anything that reads and writes lines (a pipe handle, or a buffer in tests).

use crate::protocol::{self, RpcError};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};

#[derive(Debug)]
pub enum ClientError {
    Io(std::io::Error),
    /// The server closed the connection without answering.
    NoReply,
    /// The server answered with an error.
    Rpc(RpcError),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Io(error) => write!(formatter, "connection problem: {error}"),
            ClientError::NoReply => formatter.write_str("Winuake closed the connection without answering"),
            ClientError::Rpc(error) => write!(formatter, "{}", error.message),
        }
    }
}

impl std::error::Error for ClientError {}

/// Sends `method` with `params` and returns the result.
pub fn call<T: Read + Write>(connection: T, method: &str, params: &[Value]) -> Result<Value, ClientError> {
    let mut reader = BufReader::new(connection);
    let mut line = protocol::request_line(1, method, params);
    line.push('\n');
    reader.get_mut().write_all(line.as_bytes()).map_err(ClientError::Io)?;
    reader.get_mut().flush().map_err(ClientError::Io)?;
    let mut reply = String::new();
    if reader.read_line(&mut reply).map_err(ClientError::Io)? == 0 {
        return Err(ClientError::NoReply);
    }
    protocol::parse_response(reply.trim_end()).map_err(ClientError::Rpc)
}

/// Turns a command-line word into a JSON argument: whole numbers and `true`/`false` keep their type, everything else is text.
pub fn argument_from_text(text: &str) -> Value {
    if let Ok(number) = text.parse::<i64>() {
        return Value::from(number);
    }
    match text {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(text.to_owned()),
    }
}

/// How a result is printed: bare text and numbers like `qdbus`, nothing for no value.
pub fn display_result(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;

    /// A connection whose reads return canned bytes and whose writes are captured.
    struct Fake {
        incoming: Cursor<Vec<u8>>,
        written: Vec<u8>,
    }

    impl Read for Fake {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.incoming.read(buffer)
        }
    }

    impl Write for Fake {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.written.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn fake(reply: &str) -> Fake {
        Fake { incoming: Cursor::new(reply.as_bytes().to_vec()), written: Vec::new() }
    }

    #[test]
    fn given_a_successful_reply_when_calling_then_the_result_is_returned() {
        let result = call(fake("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"0,1\"}\n"), "sessionIdList", &[]);
        assert_eq!(result.expect("ok"), json!("0,1"));
    }

    #[test]
    fn given_a_call_when_sending_then_one_request_line_is_written() {
        let mut connection = fake("{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":null}\n");
        call(&mut connection, "raiseSession", &[json!(2)]).expect("ok");
        let sent = String::from_utf8(connection.written).expect("utf8");
        assert!(sent.ends_with('\n') && sent.matches('\n').count() == 1, "{sent:?}");
        let request = protocol::parse_request(sent.trim_end()).expect("parses");
        assert_eq!((request.method.as_str(), request.params), ("raiseSession", vec![json!(2)]));
    }

    #[test]
    fn given_an_error_reply_when_calling_then_the_rpc_error_surfaces() {
        let error = call(
            fake("{\"jsonrpc\":\"2.0\",\"id\":1,\"error\":{\"code\":-32601,\"message\":\"no such method: x\"}}\n"),
            "x",
            &[],
        )
        .unwrap_err();
        assert!(matches!(&error, ClientError::Rpc(rpc) if rpc.code == protocol::METHOD_NOT_FOUND));
        assert!(error.to_string().contains("no such method"));
    }

    #[test]
    fn given_a_closed_connection_when_calling_then_no_reply_is_reported() {
        assert!(matches!(call(fake(""), "addSession", &[]), Err(ClientError::NoReply)));
        assert!(ClientError::NoReply.to_string().contains("without answering"));
    }

    #[test]
    fn given_command_line_words_when_converting_then_numbers_and_booleans_keep_their_type() {
        assert_eq!(argument_from_text("3"), json!(3));
        assert_eq!(argument_from_text("-1"), json!(-1));
        assert_eq!(argument_from_text("true"), json!(true));
        assert_eq!(argument_from_text("ls -l"), json!("ls -l"));
        assert_eq!(argument_from_text("3x"), json!("3x"));
    }

    #[test]
    fn given_results_when_displaying_then_text_is_bare_and_null_prints_nothing() {
        assert_eq!(display_result(&json!("0,1")), Some("0,1".to_owned()));
        assert_eq!(display_result(&json!(4)), Some("4".to_owned()));
        assert_eq!(display_result(&json!(false)), Some("false".to_owned()));
        assert_eq!(display_result(&Value::Null), None);
    }

    #[test]
    fn given_io_trouble_when_displaying_the_error_then_it_says_connection_problem() {
        let error = ClientError::Io(std::io::Error::other("pipe broke"));
        assert!(error.to_string().contains("connection problem"));
    }
}
