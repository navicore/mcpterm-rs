use crate::protocol::validation::{
    create_correction_prompt, validate_llm_response, ValidationResult,
};
use serde_json::json;

#[test]
fn test_create_correction_prompt_valid() {
    // Valid should return empty string
    let valid_json = json!({
        "jsonrpc": "2.0",
        "result": "Hello",
        "id": "test123"
    });
    let validation_result = ValidationResult::Valid(valid_json);
    let prompt = create_correction_prompt(&validation_result);
    assert!(
        prompt.is_empty(),
        "Correction prompt for valid JSON-RPC should be empty"
    );
}

#[test]
fn test_create_correction_prompt_invalid_format() {
    // Test invalid format correction prompt
    let invalid_text = "Hello, I'll help you with that!";
    let validation_result = ValidationResult::InvalidFormat(invalid_text.to_string());
    let prompt = create_correction_prompt(&validation_result);

    // Check that the correction prompt contains expected elements
    assert!(
        prompt.contains("not in the required JSON-RPC 2.0 format"),
        "Prompt should mention invalid format"
    );
    assert!(
        prompt.contains("\"Hello, I'll help you with that!\""),
        "Prompt should include the original text"
    );
    assert!(
        prompt.contains("jsonrpc"),
        "Prompt should include example format"
    );
}

#[test]
fn test_create_correction_prompt_mixed() {
    // Test mixed content correction prompt
    let text = "I'll help with your request.";
    let json = json!({
        "jsonrpc": "2.0",
        "result": "Here's my response",
        "id": "test123"
    });

    let validation_result = ValidationResult::Mixed {
        text: text.to_string(),
        json_rpc: Some(json.clone()),
    };

    let prompt = create_correction_prompt(&validation_result);

    // Check that the correction prompt contains expected elements
    assert!(
        prompt.contains("mixed regular text with JSON-RPC"),
        "Prompt should mention mixed content"
    );
    assert!(prompt.contains(text), "Prompt should include the text part");
    assert!(
        prompt.contains("Your JSON part was:"),
        "Prompt should reference JSON part"
    );
}

#[test]
fn test_create_correction_prompt_not_jsonrpc() {
    // Test not JSON-RPC correction prompt
    let non_jsonrpc = json!({
        "message": "This is just regular JSON",
        "foo": "bar"
    });

    let validation_result = ValidationResult::NotJsonRpc(non_jsonrpc);
    let prompt = create_correction_prompt(&validation_result);

    // Check that the correction prompt contains expected elements
    assert!(
        prompt.contains("valid JSON but not a valid JSON-RPC 2.0 object"),
        "Prompt should mention it's valid JSON but not JSON-RPC"
    );
    assert!(
        prompt.contains("message"),
        "Prompt should include original JSON content"
    );
    assert!(
        prompt.contains("jsonrpc"),
        "Prompt should include example format"
    );
}

#[test]
fn test_validate_malformed_json() {
    // Test with malformed/incomplete JSON
    let malformed = r#"{"jsonrpc": "2.0", "result": "incomplete"#;
    let result = validate_llm_response(malformed);

    match result {
        ValidationResult::InvalidFormat(_) => (),
        _ => panic!(
            "Expected InvalidFormat for malformed JSON, got {:?}",
            result
        ),
    }
}

#[test]
fn test_validate_with_whitespace_variations() {
    // Test with different whitespace/indentation
    let indented = r#"
    {
        "jsonrpc": "2.0",
        "result": "Hello, world!",
        "id": "123"
    }
    "#;

    let result = validate_llm_response(indented);

    match result {
        ValidationResult::Valid(_) => (),
        _ => panic!("Expected Valid for indented JSON-RPC, got {:?}", result),
    }
}

#[test]
fn test_validate_with_various_id_types() {
    // Test with number id
    let num_id = r#"{"jsonrpc":"2.0","result":"test","id":123}"#;
    let result = validate_llm_response(num_id);
    assert!(
        matches!(result, ValidationResult::Valid(_)),
        "Should accept numeric ID"
    );

    // Test with null id
    let null_id = r#"{"jsonrpc":"2.0","result":"test","id":null}"#;
    let result = validate_llm_response(null_id);
    assert!(
        matches!(result, ValidationResult::Valid(_)),
        "Should accept null ID"
    );
}

#[test]
fn test_validate_empty_response() {
    // Test with empty response
    let empty = "";
    let result = validate_llm_response(empty);
    assert!(
        matches!(result, ValidationResult::InvalidFormat(_)),
        "Should handle empty response as invalid"
    );
}

#[test]
fn test_validate_missing_both_result_and_error() {
    // Missing both result and error
    let missing_both = r#"{"jsonrpc":"2.0","id":"123"}"#;
    let result = validate_llm_response(missing_both);
    assert!(
        matches!(result, ValidationResult::NotJsonRpc(_)),
        "Should reject JSON-RPC missing both result and error"
    );
}

#[test]
fn test_validate_multiple_json_objects() {
    // Multiple JSON objects in one response
    let multiple = r#"{"jsonrpc":"2.0","result":"first","id":"1"} {"jsonrpc":"2.0","result":"second","id":"2"}"#;
    let result = validate_llm_response(multiple);
    assert!(
        matches!(result, ValidationResult::InvalidFormat(_)),
        "Should handle multiple JSON objects as invalid"
    );
}

#[test]
fn test_validate_with_unicode() {
    // Test with Unicode content
    let unicode = r#"{"jsonrpc":"2.0","result":"こんにちは、世界！","id":"123"}"#;
    let result = validate_llm_response(unicode);
    assert!(
        matches!(result, ValidationResult::Valid(_)),
        "Should handle Unicode content"
    );
}
