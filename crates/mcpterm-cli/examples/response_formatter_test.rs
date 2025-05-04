use mcpterm_cli::formatter::format_llm_response;

fn main() {
    // Test various response formats and verify the formatter extracts the correct content

    // Test 1: Simple text (non-JSON)
    let simple_text = "This is a simple text response";
    println!("=== Test 1: Simple Text ===");
    println!("Input: {}", simple_text);
    println!("Output: {}", format_llm_response(simple_text));
    println!();

    // Test 2: LlmResponse format with content field
    let llm_response = r#"{"id":"1234","content":"This is the content from LlmResponse","tool_calls":[]}"#;
    println!("=== Test 2: LlmResponse with content field ===");
    println!("Input: {}", llm_response);
    println!("Output: {}", format_llm_response(llm_response));
    println!();

    // Test 3: JSON-RPC Response with result field
    let jsonrpc_response = r#"{"jsonrpc":"2.0","result":{"content":"This is the result from JSON-RPC"},"id":"5678"}"#;
    println!("=== Test 3: JSON-RPC Response with result field ===");
    println!("Input: {}", jsonrpc_response);
    println!("Output: {}", format_llm_response(jsonrpc_response));
    println!();

    // Test 4: Invalid JSON
    let invalid_json = "{not valid json}";
    println!("=== Test 4: Invalid JSON ===");
    println!("Input: {}", invalid_json);
    println!("Output: {}", format_llm_response(invalid_json));
    println!();

    // Test 5: JSON without expected fields
    let json_no_fields = r#"{"some_field":"some value","other_field":123}"#;
    println!("=== Test 5: JSON without expected fields ===");
    println!("Input: {}", json_no_fields);
    println!("Output: {}", format_llm_response(json_no_fields));
    println!();

    // Test 6: Formatted text with whitespace and newlines
    let formatted_text = "This text has formatting\n  with indentation\n    and multiple lines\nthat should be preserved";
    println!("=== Test 6: Formatted text ===");
    println!("Input: \n{}", formatted_text);
    println!("Output: \n{}", format_llm_response(formatted_text));
    println!();

    // Test 7: LlmResponse with formatted content
    let formatted_llm_response = r#"{"id":"1234","content":"This text has formatting\n  with indentation\n    and multiple lines\nthat should be preserved","tool_calls":[]}"#;
    println!("=== Test 7: LlmResponse with formatted content ===");
    println!("Input: {}", formatted_llm_response);
    println!("Output: \n{}", format_llm_response(formatted_llm_response));
    println!();
    
    // Test 8: Complex nested JSON structure (matches actual response structure)
    let complex_response = r#"{"jsonrpc":"2.0","result":{"id":"response-123","content":"Here's the actual result that should be displayed to the user\nWith proper formatting preserved"},"id":"987"}"#;
    println!("=== Test 8: Complex nested JSON structure ===");
    println!("Input: {}", complex_response);
    println!("Output: \n{}", format_llm_response(complex_response));
    println!();
    
    // Test 9: LLM returning a JSON-RPC response inside content field
    let nested_jsonrpc = r#"{"id":"1234","content":"{\"jsonrpc\": \"2.0\", \"result\": \"This is the extracted result text\", \"id\": \"inner-id\"}","tool_calls":[]}"#;
    println!("=== Test 9: LLM returning JSON-RPC inside content ===");
    println!("Input: {}", nested_jsonrpc);
    println!("Output: \n{}", format_llm_response(nested_jsonrpc));
    println!();
    
    // Test 10: Actual failing case from the real app (raw JSON in result)
    let real_case = r#"{"id":"123","content":"{\"jsonrpc\": \"2.0\", \"result\": \"Based on the README.md file, MCPTerm-RS is a terminal-based client for the Model Context Protocol (MCP) written in Rust. Here are my observations:\\n\\n1. **Well-Structured Architecture**: The project uses a modular workspace-based structure with seven distinct crates, each with a clear responsibility.\\n\\n2. **Development Status**: The project appears to be in early stages with basic structure set up.\\n\\n3. **User-Friendly**: The documentation includes clear information for users.\", \"id\": \"request_1\"}","tool_calls":[]}"#;
    
    // Test 11: The exact format seen in the failing example
    let exact_failing_case = r#"{"jsonrpc":"2.0","result":"The file contains an MIT License, which is one of the most permissive and widely-used open source licenses. Here are the key points about this license:\n\n1. Copyright is held by Ed Sweeney with a future date of 2025 (which appears to be a placeholder or forward-dated copyright).\n\n2. The MIT License grants very liberal permissions to users of the software, allowing them to:\n   - Use the software without restriction\n   - Modify the code\n   - Distribute the software\n   - Use it commercially\n   - Sublicense it\n\n3. The only real requirement is that the copyright notice and permission notice must be included in all copies or substantial portions of the software.\n\n4. The license includes a standard disclaimer of warranty and liability.\n\nThe MIT License is business-friendly and compatible with many other licenses, making it an excellent choice for open source projects where wide adoption and minimal restrictions are desired.","id":1}"#;
    
    // Test 12: The actual Claude response structure from Bedrock API
    let actual_claude_structure = r#"{"id":"msg_123","content":[{"type":"text","text":"{\n  \"jsonrpc\": \"2.0\",\n  \"result\": \"The file contains an MIT License with copyright held by Ed Sweeney (2025).\",\n  \"id\": \"1\"\n}"}],"model":"claude-3-7-sonnet-20250219","role":"assistant"}"#;
    println!("=== Test 10: Real failing case ===");
    println!("Input: {}", real_case);
    println!("Output: \n{}", format_llm_response(real_case));
    println!();
    
    println!("=== Test 11: Exact failing format ===");
    println!("Input: {}", exact_failing_case);
    println!("Output: \n{}", format_llm_response(exact_failing_case));
    println!();
    
    println!("=== Test 12: Actual Claude Bedrock structure ===");
    println!("Input: {}", actual_claude_structure);
    println!("Output: \n{}", format_llm_response(actual_claude_structure));
    println!();
}