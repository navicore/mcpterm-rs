# Clean Implementation Integration Plan

This document outlines the plan for integrating the clean TUI implementation with the LLM client and other features of the mcpterm-tui application.

## Current State

The clean implementation (`--clean-mode`) provides a solid foundation with:

- Reliable keyboard input handling
- Proper focus management between input and messages
- Working message scrolling with j/k keys
- VI-style input editing with normal and insert modes
- Clean, responsive UI rendering

However, it lacks integration with:
- LLM client for message processing
- Tool execution capabilities
- History and context management
- Status updates and processing indicators

## Integration Approach

### Phase 1: Basic LLM Integration

1. Add LLM client initialization in the clean implementation
   ```rust
   // Add LLM client to SimpleApp
   struct SimpleApp {
       state: AppState,
       needs_redraw: bool,
       llm_client: Option<Box<dyn LlmClient>>,
   }
   ```

2. Create a simple channel for LLM responses
   ```rust
   // Create a channel for communication
   let (tx, rx) = std::sync::mpsc::channel();
   ```

3. Add message processing with basic async handling
   ```rust
   // In handle_key for Enter key
   if let Some(input) = self.state.submit_input() {
       if let Some(client) = &self.llm_client {
           // Process async in a separate thread
           let tx_clone = tx.clone();
           let context_clone = self.state.context.clone();
           let client_clone = client.clone_box();
           
           std::thread::spawn(move || {
               // Process the message
               let result = client_clone.process(input, context_clone);
               
               // Send result back to main thread
               let _ = tx_clone.send(result);
           });
       }
   }
   ```

4. Check for LLM responses in the main loop
   ```rust
   // In the main loop
   // Check for LLM responses
   if let Ok(result) = rx.try_recv() {
       app.state.process_llm_response(result);
       app.needs_redraw = true;
   }
   ```

### Phase 2: Tool Execution Integration

1. Add tool registry and execution capabilities
   ```rust
   struct SimpleApp {
       state: AppState,
       needs_redraw: bool,
       llm_client: Option<Box<dyn LlmClient>>,
       tool_registry: ToolRegistry,
   }
   ```

2. Process tool calls from LLM responses
   ```rust
   fn process_tool_calls(&mut self, tool_calls: Vec<ToolCall>) {
       for tool_call in tool_calls {
           // Execute the tool
           let result = self.tool_registry.execute(tool_call.tool, tool_call.params);
           
           // Add result to the conversation
           match result {
               Ok(output) => {
                   self.state.add_message(format!("Tool result: {}", output), MessageType::Tool);
               }
               Err(e) => {
                   self.state.add_message(format!("Tool error: {}", e), MessageType::Error);
               }
           }
       }
   }
   ```

### Phase 3: Status and Visuals

1. Add status indicators for processing state
   ```rust
   fn render_status(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
       let status = match self.state.processing {
           ProcessingStatus::Idle => "Idle",
           ProcessingStatus::Connecting => "Connecting...",
           ProcessingStatus::Processing { .. } => "Processing...",
           ProcessingStatus::Error(ref msg) => "Error",
       };
       
       // Render status widget
       let status_widget = Paragraph::new(status)
           .block(Block::default().title("Status").borders(Borders::ALL));
       
       f.render_widget(status_widget, area);
   }
   ```

2. Implement progress indicators
   ```rust
   // Update processing status with a periodic timer
   if let ProcessingStatus::Processing { start_time, .. } = self.state.processing {
       let elapsed = start_time.elapsed();
       self.state.update_processing_status(format!("Processing... ({:?})", elapsed));
       app.needs_redraw = true;
   }
   ```

### Phase 4: History and UI Enhancements

1. Add input history navigation
   ```rust
   // In handle_key
   KeyCode::Up if self.state.focus == FocusArea::Input => {
       if let Some(prev) = self.state.input_history.previous(&self.state.input_content) {
           self.state.input_content = prev;
           self.state.input_cursor = self.state.input_content.len();
       }
       return true;
   }
   ```

2. Add more advanced UI features like syntax highlighting
   ```rust
   fn render_input(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
       // Add syntax highlighting for commands
       let styled_input = highlight_syntax(&self.state.input_content);
       
       // Create paragraph widget with highlighting
       let paragraph = Paragraph::new(styled_input)
           .block(block)
           .wrap(Wrap { trim: true });
       
       f.render_widget(paragraph, area);
   }
   ```

## Implementation Steps

1. Start with the clean implementation and verify it works correctly
2. Add LLM client initialization and basic async processing
3. Add tool execution capabilities
4. Enhance UI with status indicators and progress updates
5. Add history navigation and advanced UI features
6. Test thoroughly to ensure keyboard handling remains reliable

## Benefits of This Approach

1. Start with a clean slate that has proven keyboard handling
2. Build features incrementally to avoid reintroducing bugs
3. Focus on user experience first, then add integrations
4. Separate concerns between UI and async operations

## Conclusion

By starting with the clean implementation and building integrations piece by piece, we can maintain the solid keyboard handling foundation while adding back all the necessary features. This approach will lead to a more reliable and user-friendly TUI application.