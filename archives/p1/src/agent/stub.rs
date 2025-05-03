use super::Agent;

#[derive(Clone)]
pub struct StubAgent;

impl StubAgent {
    pub fn new() -> Self {
        Self
    }
}

impl Agent for StubAgent {
    fn process_message(&self, input: &str) -> String {
        // Simple echo agent for initial development
        format!("Echo: {}", input)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

impl Default for StubAgent {
    fn default() -> Self {
        Self::new()
    }
}
