#[cfg(test)]
mod tests {
    use crate::mcp::protocol::error::{Error, ErrorCode};
    use crate::mcp::protocol::message::{Id, Request, Response, Version, JSONRPC_VERSION};
    use serde_json::{json, Value};

    #[test]
    fn test_request_serialization() {
        let request = Request {
            jsonrpc: Version(JSONRPC_VERSION.to_string()),
            method: "test_method".to_string(),
            params: Some(json!({
                "param1": "value1",
                "param2": 42
            })),
            id: Some(Id::Number(1)),
        };

        let json = serde_json::to_string(&request).unwrap();
        let expected = r#"{"jsonrpc":"2.0","method":"test_method","params":{"param1":"value1","param2":42},"id":1}"#;

        assert_eq!(json, expected);
    }

    #[test]
    fn test_request_deserialization() {
        let json = r#"{"jsonrpc":"2.0","method":"test_method","params":{"param1":"value1","param2":42},"id":1}"#;
        let request: Request = serde_json::from_str(json).unwrap();

        assert_eq!(request.jsonrpc.0, JSONRPC_VERSION);
        assert_eq!(request.method, "test_method");

        if let Some(params) = request.params {
            assert_eq!(params["param1"], "value1");
            assert_eq!(params["param2"], 42);
        } else {
            panic!("Expected params to be Some");
        }

        assert_eq!(request.id, Some(Id::Number(1)));
    }

    #[test]
    fn test_success_response_serialization() {
        let response = Response::success(json!({"result_key": "result_value"}), Id::Number(1));
        let json = serde_json::to_string(&response).unwrap();

        let expected = r#"{"jsonrpc":"2.0","result":{"result_key":"result_value"},"id":1}"#;
        assert_eq!(json, expected);
    }

    #[test]
    fn test_error_response_serialization() {
        let error = Error::new(
            ErrorCode::InvalidParams as i32,
            "Invalid params".to_string(),
            Some(json!(["param1", "param2"])),
        );

        let response = Response::error(error, Id::Number(1));
        let json = serde_json::to_string(&response).unwrap();

        // The order of fields in the error object might vary, so we'll deserialize and check fields
        let parsed: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["error"]["code"], ErrorCode::InvalidParams as i32);
        assert_eq!(parsed["error"]["message"], "Invalid params");
        assert_eq!(parsed["error"]["data"], json!(["param1", "param2"]));
        assert_eq!(parsed["id"], 1);
    }

    #[test]
    fn test_notification_request() {
        let request = Request {
            jsonrpc: Version(JSONRPC_VERSION.to_string()),
            method: "notification_method".to_string(),
            params: Some(json!({"event": "something_happened"})),
            id: None,
        };

        assert!(request.is_notification());

        let json = serde_json::to_string(&request).unwrap();

        // Deserializing an object with null ID should work
        let deserialized: Request = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_notification());
    }
}
