/// Represents the result of handling a key event in the editor
#[derive(Debug, Clone)]
pub enum HandleResult {
    /// Continue editing, no special action needed
    Continue,
    /// Submit the current text (e.g., Enter was pressed)
    Submit(String),
    /// Abort the current editing (e.g., Escape in certain contexts)
    Abort,
}
