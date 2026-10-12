//! Provider-neutral conversation types.

use serde_json::Value;

/// A tool the model may call: its name, what it does, and its JSON-schema parameters.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

/// One entry of a conversation.
#[derive(Debug, Clone)]
pub enum Turn {
    User(String),
    /// A model reply. `raw` is the message exactly as the provider returned it and is resent
    /// verbatim: providers attach hidden reasoning/signature data that must come back unchanged.
    Assistant { text: String, calls: Vec<ToolCall>, raw: Value },
    ToolResult { call_id: String, name: String, content: String, is_error: bool },
}

/// Token counts reported by the provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Part of the input served from the provider's prompt cache (cheaper).
    pub cached_tokens: u64,
}

impl std::ops::AddAssign for Usage {
    fn add_assign(&mut self, o: Usage) {
        self.input_tokens += o.input_tokens;
        self.output_tokens += o.output_tokens;
        self.cached_tokens += o.cached_tokens;
    }
}

/// One model response.
#[derive(Debug, Clone)]
pub struct Reply {
    pub text: String,
    pub calls: Vec<ToolCall>,
    pub raw: Value,
    pub usage: Usage,
    /// The provider stopped at its output limit.
    pub truncated: bool,
}
