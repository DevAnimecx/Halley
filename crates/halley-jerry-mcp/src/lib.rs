//! MCP-based browser tool protocol.
//!
//! Prompt #5: **read-only tool contracts** only — ids, risk tiers, JSON
//! schemas, and a registry. There is **no executor** and no autonomous
//! agent loop in this milestone (see ADR-005).
//!
//! Dependencies: none beyond the workspace (bottom crate with common if
//! needed later; currently std + serde for schema JSON).

pub mod registry;
pub mod tool;
pub mod tools;

pub use registry::ToolRegistry;
pub use tool::{RiskLevel, ToolCall, ToolDefinition, ToolId, ToolResult, ToolStatus};
pub use tools::read_only_tools;

/// Identifier for this subsystem, used by diagnostics and logging.
pub const SUBSYSTEM: &str = "halley-jerry-mcp";
