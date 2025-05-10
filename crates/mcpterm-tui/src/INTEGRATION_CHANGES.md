# Integration Changes for UI Improvements

This document outlines the specific changes needed to integrate our working solution with improved scrolling and focus management into the main codebase.

## Issues to Fix

1. **Message Scrolling**
   - Currently, new messages aren't always visible as they're added
   - j/k navigation works inconsistently
   - There's no auto-scroll functionality

2. **Focus Management**
   - Tab key sometimes requires multiple presses
   - When messages have focus, j/k keys don't consistently work for scrolling

3. **UI Display**
   - Messages don't use the full available area
   - The UI doesn't provide clear feedback on auto-scroll status

## Specific Changes

### 1. Update AppState in `state/mod.rs`

```rust
// Add auto_scroll flag to AppState
pub struct AppState {
    // ... existing fields
    pub messages_scroll: usize,
    pub auto_scroll: bool,  // Add this field
    // ... rest of fields
}

impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing fields
            messages_scroll: 0,
            auto_scroll: true,  // Enable auto-scroll by default
            // ... rest of fields
        }
    }
    
    // Modify the add_message method to respect auto-scroll
    pub fn add_message(&mut self, content: String, message_type: MessageType) {
        // Create and add the message
        let message = Message::new(content, message_type);
        self.messages.push(message.clone());
        self.message_count += 1;
        
        // If auto-scroll is enabled, reset scroll position to see the new message
        if self.auto_scroll {
            self.messages_scroll = 0;
        }
        
        // ... rest of method (context update, etc.)
    }
    
    // Add method to toggle auto-scroll
    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
        if self.auto_scroll {
            self.messages_scroll = 0;
        }
    }
}
```

### 2. Update Key Handling in `handle_key_event` in `state/mod.rs`

```rust
// Add 'a' key to toggle auto-scroll when messages are focused
match (self.focus, self.editor_mode, key.code) {
    // ... existing key handlers
    
    // Toggle auto-scroll
    (FocusArea::Messages, _, KeyCode::Char('a')) => {
        self.toggle_auto_scroll();
        true
    },
    
    // ... other key handlers
}
```

### 3. Update Message Viewer Rendering in `ui/mod.rs`

```rust
fn render_messages(f: &mut ratatui::Frame, state: &mut AppState, area: Rect) {
    // Create a block with borders
    let auto_scroll_status = if state.auto_scroll { "AUTO" } else { "MANUAL" };
    let title = format!("Messages (scroll: {}/{} - {})", 
        state.messages_scroll, 
        state.messages.len().saturating_sub(1),
        auto_scroll_status);
        
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if state.focus == FocusArea::Messages {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        });
    
    // Get access to the message viewer
    static mut MESSAGE_VIEWER: Option<MessageViewer> = None;
    
    let message_viewer = unsafe {
        if MESSAGE_VIEWER.is_none() {
            info!("Initializing MESSAGE_VIEWER static");
            let mut viewer = MessageViewer::new();
            // Start in normal mode for navigation
            viewer.set_mode(EditorMode::Normal);
            MESSAGE_VIEWER = Some(viewer);
            info!("MESSAGE_VIEWER initialized");
        } else {
            info!("Using existing MESSAGE_VIEWER");
        }
        MESSAGE_VIEWER.as_mut().unwrap()
    };
    
    // Set block based on focus
    message_viewer.block = Some(block);
    
    // Apply messages with reverse order for scrolling (most recent at the bottom)
    let messages_offset = state.messages_scroll;
    let messages_to_show = if messages_offset >= state.messages.len() {
        &[]
    } else {
        // Show all messages from start to end minus offset
        &state.messages[0..state.messages.len() - messages_offset]
    };
    
    // Update content
    message_viewer.set_content(messages_to_show);
    
    // Render the message viewer
    f.render_widget(message_viewer, area);
}
```

### 4. Modify Message Viewer Implementation in `ui/message_viewer.rs`

The current message viewer is using edtui for read-only viewing, but this adds complexity without a clear benefit. For scrolling reliability, we should consider simplifying the message viewer to use a basic Paragraph widget.

This is a more significant change and would involve:

1. Creating a new implementation based on Paragraph
2. Adding support for styled message rendering
3. Ensuring backward compatibility with existing APIs

### 5. Update Direct Key Handling in `lib.rs`

```rust
// In the run_event_loop method
match event_handler.next()? {
    Event::Input(key) => {
        // Handle Tab key as before - ALWAYS handle this separately
        if key.code == crossterm::event::KeyCode::Tab {
            // ... existing Tab handling
        }
        
        // Handle focus-specific keys
        match self.state.focus {
            FocusArea::Messages => {
                // Process keys specifically for navigation in message viewer
                let result = match key.code {
                    crossterm::event::KeyCode::Char('j') => {
                        info!("  Message viewer: 'j' key - move down");
                        if self.state.messages_scroll > 0 {
                            self.state.messages_scroll -= 1;
                            info!("  Scrolled messages down, offset: {}", self.state.messages_scroll);
                        }
                        ViewerHandleResult::Continue
                    },
                    crossterm::event::KeyCode::Char('k') => {
                        info!("  Message viewer: 'k' key - move up");
                        if self.state.messages_scroll < self.state.messages.len() {
                            self.state.messages_scroll += 1;
                            info!("  Scrolled messages up, offset: {}", self.state.messages_scroll);
                        }
                        ViewerHandleResult::Continue
                    },
                    crossterm::event::KeyCode::Char('a') => {
                        info!("  Message viewer: 'a' key - toggle auto-scroll");
                        self.state.toggle_auto_scroll();
                        self.state.add_message(
                            format!("Auto-scroll {}", if self.state.auto_scroll { "enabled" } else { "disabled" }),
                            MessageType::System,
                        );
                        ViewerHandleResult::Continue
                    },
                    // Other message viewer keys
                    _ => {
                        info!("  Message viewer: passing key to editor component");
                        // Pass to the editor component
                        message_viewer.handle_key_event(key)
                    }
                };
                
                // Process the result as before
                // ...
            },
            FocusArea::Input => {
                // Input handling continues as before
                // ...
            }
        }
    },
    // Other events continue as before
    // ...
}
```

## Implementation Strategy

1. **Make Minimal Changes First**
   - Add the auto_scroll flag and toggle functionality
   - Adjust the message display logic to show more messages
   - Fix j/k navigation in the message viewer

2. **Test Each Change Incrementally**
   - Verify that Tab key navigation works consistently
   - Ensure that j/k scrolling works reliably when messages have focus
   - Confirm that new messages are visible when added

3. **Consider Simplifying the Message Viewer**
   - If there are still issues, consider replacing the edtui-based message viewer with a simpler implementation
   - This would be a larger change and should be done after the simpler fixes are tested

## Risks and Mitigation

1. **Complex Event Handling**
   - The current event handling is complex with multiple layers
   - Mitigation: Focus on the most critical issues first and make targeted changes

2. **Static Message Viewer**
   - The current message viewer is stored in a static variable, which can complicate state management
   - Mitigation: Consider moving to a non-static approach similar to our example implementation

3. **Breaking API Changes**
   - Some changes might affect the API expected by other parts of the codebase
   - Mitigation: Maintain backward compatibility where possible; where not possible, update all affected code

By following this plan, we can integrate the key improvements from our working solution while minimizing disruption to the existing codebase.