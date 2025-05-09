# Rebuilding mcpterm-tui from Scratch

This directory contains a step-by-step rebuild of the mcpterm-tui application, focusing on reliable keyboard input handling.

## The Problem

The current implementation has major issues with keyboard input:
- Tab key requires multiple presses to change focus
- j/k keys in message viewer only work when input has focus
- Keys sometimes affect the wrong component

These issues suggest a fundamental design problem in how keyboard input is handled and distributed to components.

## Rebuild Strategy

We're rebuilding the UI from scratch with minimal dependencies:

1. **Start with the absolute minimum** - Verify raw keyboard input works
2. **Add features incrementally** - Test each feature before moving on
3. **Use direct terminal handling** - Avoid complex component architectures
4. **Maintain clear separation of concerns** - One place for key handling

## Examples

### Step 1: Minimal Keyboard Handling
`step1_minimal.rs` - A minimal terminal application that just echoes key presses.
This verifies basic keyboard input works correctly.

```
cargo run --example step1_minimal
```

### Step 2: Basic Two-Panel Layout
`step2_two_panels.rs` - A simple two-panel layout with message area and input area.
- Tab key switches focus between panels
- Enter submits input
- Basic message display

```
cargo run --example step2_two_panels
```

### Step 3: VI-Style Mode Support
`step3_with_modes.rs` - Adds VI-style normal/insert modes
- Tab key switches focus
- i key enters insert mode
- Esc returns to normal mode
- j/k keys scroll messages when message panel has focus

```
cargo run --example step3_with_modes
```

## Design Principles

1. **Single Source of Truth**
   - All state is kept in one place (AppState)
   - No shared mutable state between components

2. **Explicit Focus Management**
   - Focus is a first-class concept in the state
   - Tab key is handled in one place only
   - All keyboard handling respects focus state

3. **Clear Key Handling Hierarchy**
   - Global keys first (Tab, Esc, q)
   - Focus-specific keys next
   - Mode-specific keys last

4. **Direct Rendering from State**
   - UI is rendered directly from state
   - No bidirectional state synchronization
   - No component-local state

## Integration Plan

After validating this approach:

1. Port the minimal working version to use ratatui for rendering
2. Integrate a minimal edtui component for the input area
3. Gradually rebuild the messaging functionality
4. Add back the async message processing

The goal is to maintain the clean architecture and reliable keyboard handling from our minimal examples, while leveraging the UI components from ratatui and edtui.

## Testing

Each step should be tested thoroughly to ensure:
1. Tab key works reliably with a single press
2. j/k navigation works when message panel has focus
3. Input works correctly in insert mode
4. Mode switching with i/Esc works as expected

Test both inside and outside tmux to verify terminal handling.