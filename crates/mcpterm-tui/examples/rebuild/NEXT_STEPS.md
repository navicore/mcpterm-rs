# Next Steps for mcpterm-tui Rebuild

Now that we've created a series of working examples that demonstrate reliable keyboard input handling, here are the next steps to rebuild the main application:

## Implementation Plan

1. **Start with a New Implementation**

   Begin with a clean implementation based on `final_implementation.rs` rather than trying to patch the existing code. The fundamental architecture issues need a fresh start.

2. **Integration Steps**

   a. **Port Basic Structure**
   - Copy the direct keyboard handling approach from our final example
   - Maintain clear focus and mode state management
   - Keep the clear key handling hierarchy

   b. **Adapt Application State**
   - Port the existing app state fields (messages, context, etc.)
   - Use the clean state management approach from our examples
   - Keep all state in one place to avoid synchronization issues

   c. **Add Async Message Processing**
   - Carefully integrate the async message processing after the basic UI works
   - Use a message queue pattern to avoid direct state modification from background tasks
   - Keep UI state updates in the main thread

   d. **Reconnect to Backend**
   - After the UI is stable, reconnect to the backend services
   - Add tool execution capabilities
   - Implement LLM response handling

## Key Architectural Principles

1. **Direct Keyboard Handling**
   - Poll for events directly in the main loop
   - Handle them based on the clear hierarchy:
     1. Global keys (Tab, Esc, Ctrl+C)
     2. Focus-specific keys
     3. Mode-specific keys
   - Only pass keys to edtui components when appropriate

2. **Single Source of Truth**
   - Keep all state in the AppState struct
   - Avoid static variables and shared mutable state
   - Synchronize UI components with state, not the other way around

3. **Explicit Component Boundaries**
   - UI components render state but don't modify it
   - Input components receive keys but state changes happen at the App level
   - Clear separation between rendering and state management

4. **Simplified Event Flow**
   ```
   Keyboard/Network Events → Main Loop → State Updates → UI Rendering
   ```

5. **Graceful Terminal Handling**
   - Add robust error handling for terminal operations
   - Detect TTY capabilities and adjust behavior
   - Provide fallbacks for error cases

## Testing Strategy

For each integration step:

1. **Test Focus Management**
   - Verify Tab key works with a single press
   - Check that focus changes are immediately visible

2. **Test Navigation**
   - Verify j/k keys work in message viewer when focused
   - Check that navigation controls respond correctly

3. **Test Mode Switching**
   - Verify i/Esc/v keys change modes correctly
   - Check that mode-specific behaviors work appropriately

4. **Test Input/Output**
   - Verify message submission works
   - Check that async responses appear correctly

## Implementation Details

Start by implementing this simplified architecture in a new file:

```rust
// mcpterm_tui/src/direct_impl.rs

// 1. Port the basic structure from final_implementation.rs
// 2. Add specific mcpterm-tui features gradually
// 3. Test focus and keyboard handling at each step
```

Then create a feature flag in Cargo.toml to switch between implementations:

```toml
[features]
direct_implementation = []
```

And update the main entry point to use the new implementation when the feature is enabled:

```rust
#[cfg(feature = "direct_implementation")]
use crate::direct_impl::run as run_app;

#[cfg(not(feature = "direct_implementation"))]
use crate::lib::App;

fn main() -> Result<()> {
    #[cfg(feature = "direct_implementation")]
    {
        run_app()
    }
    
    #[cfg(not(feature = "direct_implementation"))]
    {
        let mut app = App::new()?;
        app.run()
    }
}
```

This approach allows gradual migration while keeping the existing implementation available.

## Conclusion

By rebuilding from the ground up with these principles, we'll create a much more reliable terminal UI that handles keyboard input correctly. The direct keyboard handling approach with clear focus management will resolve the Tab key and navigation issues while providing a solid foundation for the application.

The examples we've created provide a proven path forward - each one demonstrates that these principles work in practice and lead to reliable keyboard handling.