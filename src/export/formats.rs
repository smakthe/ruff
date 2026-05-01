//! Export and import format definitions

use crate::{
    session::manager::{ChatSession, Message, MessageRole},
    EnhancedError,
};
use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, EnhancedError>;

/// Export format enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExportFormat {
    Markdown,
    PlainText,
    Json,
    Html,
}

/// Import format enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ImportFormat {
    Json,
    ChatGptExport,
    ClaudeExport,
}

/// Export metadata options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub include_timestamps: bool,
    pub include_model_info: bool,
    pub include_token_usage: bool,
    pub include_metadata: bool,
    pub pretty_format: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_timestamps: true,
            include_model_info: true,
            include_token_usage: true,
            include_metadata: false,
            pretty_format: true,
        }
    }
}

/// Format handler trait for different export/import formats
pub trait FormatHandler {
    /// Export session with messages provided separately (single source of truth pattern)
    fn export_session_with_messages(
        &self,
        session: &ChatSession,
        messages: &[Message],
        options: &ExportOptions,
    ) -> Result<String>;

    fn export_messages(&self, messages: &[Message], options: &ExportOptions) -> Result<String>;

    /// Deprecated: Use export_session_with_messages instead
    #[deprecated(note = "Use export_session_with_messages to pass messages explicitly")]
    fn export_session(&self, _session: &ChatSession, _options: &ExportOptions) -> Result<String> {
        Err(EnhancedError::unknown(
            "export_session is deprecated, use export_session_with_messages",
        ))
    }
}

/// Markdown format handler
pub struct MarkdownHandler;

impl FormatHandler for MarkdownHandler {
    fn export_session_with_messages(
        &self,
        session: &ChatSession,
        messages: &[Message],
        options: &ExportOptions,
    ) -> Result<String> {
        let mut output = String::new();

        // Session header
        output.push_str(&format!("# {}\n\n", session.title));

        if options.include_metadata {
            output.push_str("## Session Information\n\n");
            output.push_str(&format!(
                "- **Created:** {}\n",
                session.created_at.format("%Y-%m-%d %H:%M:%S")
            ));
            output.push_str(&format!(
                "- **Updated:** {}\n",
                session.updated_at.format("%Y-%m-%d %H:%M:%S")
            ));
            output.push_str(&format!("- **Model:** {}\n", session.model));

            if let Some(ref system_prompt) = session.system_prompt {
                output.push_str(&format!("- **System Prompt:** {}\n", system_prompt));
            }

            if options.include_token_usage {
                output.push_str(&format!(
                    "- **Total Tokens:** {}\n",
                    session.total_tokens_used.total_tokens
                ));
                output.push_str(&format!(
                    "- **Input Tokens:** {}\n",
                    session.total_tokens_used.input_tokens
                ));
                output.push_str(&format!(
                    "- **Output Tokens:** {}\n",
                    session.total_tokens_used.output_tokens
                ));
            }

            if !session.tags.is_empty() {
                output.push_str(&format!("- **Tags:** {}\n", session.tags.join(", ")));
            }

            output.push_str("\n---\n\n");
        }

        // Messages (passed as parameter - single source of truth)
        output.push_str("## Conversation\n\n");
        self.export_messages(messages, options)
            .map(|messages_content| {
                output.push_str(&messages_content);
                output
            })
    }

    fn export_messages(&self, messages: &[Message], options: &ExportOptions) -> Result<String> {
        let mut output = String::new();

        for message in messages {
            // Role header
            let role_header = match message.role {
                MessageRole::User => "### 👤 User",
                MessageRole::Assistant => "### 🤖 Assistant",
                MessageRole::System => "### ⚙️ System",
            };

            output.push_str(role_header);

            if options.include_timestamps {
                output.push_str(&format!(
                    " *({})* ",
                    message.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
            }

            if options.include_model_info && matches!(message.role, MessageRole::Assistant) {
                output.push_str(&format!(" *[{}]* ", message.metadata.model_used));
            }

            output.push_str("\n\n");

            // Message content
            output.push_str(&message.content);
            output.push_str("\n\n");

            // Additional metadata
            if options.include_metadata {
                let mut metadata_parts = Vec::new();

                if let Some(ref token_usage) = message.token_usage {
                    metadata_parts.push(format!("Tokens: {}", token_usage.total_tokens));
                }

                if message.metadata.response_time_ms > 0 {
                    metadata_parts.push(format!(
                        "Response time: {}ms",
                        message.metadata.response_time_ms
                    ));
                }

                if message.metadata.is_regenerated {
                    metadata_parts.push(format!(
                        "Regenerated {} times",
                        message.metadata.regeneration_count
                    ));
                }

                if message.edited_at.is_some() {
                    metadata_parts.push("Edited".to_string());
                }

                if !metadata_parts.is_empty() {
                    output.push_str(&format!("*{}*\n\n", metadata_parts.join(" • ")));
                }
            }

            output.push_str("---\n\n");
        }

        Ok(output)
    }
}

/// Plain text format handler
pub struct PlainTextHandler;

impl FormatHandler for PlainTextHandler {
    fn export_session_with_messages(
        &self,
        session: &ChatSession,
        messages: &[Message],
        options: &ExportOptions,
    ) -> Result<String> {
        let mut output = String::new();

        // Session header
        output.push_str(&format!("{}\n", session.title));
        output.push_str(&"=".repeat(session.title.len()));
        output.push_str("\n\n");

        if options.include_metadata {
            output.push_str("Session Information:\n");
            output.push_str(&format!(
                "Created: {}\n",
                session.created_at.format("%Y-%m-%d %H:%M:%S")
            ));
            output.push_str(&format!(
                "Updated: {}\n",
                session.updated_at.format("%Y-%m-%d %H:%M:%S")
            ));
            output.push_str(&format!("Model: {}\n", session.model));

            if let Some(ref system_prompt) = session.system_prompt {
                output.push_str(&format!("System Prompt: {}\n", system_prompt));
            }

            if options.include_token_usage {
                output.push_str(&format!(
                    "Total Tokens: {}\n",
                    session.total_tokens_used.total_tokens
                ));
            }

            if !session.tags.is_empty() {
                output.push_str(&format!("Tags: {}\n", session.tags.join(", ")));
            }

            output.push_str("\n");
            output.push_str(&"-".repeat(50));
            output.push_str("\n\n");
        }

        // Messages (passed as parameter - single source of truth)
        self.export_messages(messages, options)
            .map(|messages_content| {
                output.push_str(&messages_content);
                output
            })
    }

    fn export_messages(&self, messages: &[Message], options: &ExportOptions) -> Result<String> {
        let mut output = String::new();

        for (i, message) in messages.iter().enumerate() {
            // Role and timestamp
            let role_name = match message.role {
                MessageRole::User => "USER",
                MessageRole::Assistant => "ASSISTANT",
                MessageRole::System => "SYSTEM",
            };

            output.push_str(&format!("[{}]", role_name));

            if options.include_timestamps {
                output.push_str(&format!(
                    " {}",
                    message.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
            }

            if options.include_model_info && matches!(message.role, MessageRole::Assistant) {
                output.push_str(&format!(" ({})", message.metadata.model_used));
            }

            output.push_str(":\n");

            // Message content
            output.push_str(&message.content);
            output.push_str("\n");

            // Additional metadata
            if options.include_metadata {
                let mut metadata_parts = Vec::new();

                if let Some(ref token_usage) = message.token_usage {
                    metadata_parts.push(format!("Tokens: {}", token_usage.total_tokens));
                }

                if message.metadata.response_time_ms > 0 {
                    metadata_parts.push(format!(
                        "Response time: {}ms",
                        message.metadata.response_time_ms
                    ));
                }

                if message.metadata.is_regenerated {
                    metadata_parts.push(format!(
                        "Regenerated {} times",
                        message.metadata.regeneration_count
                    ));
                }

                if !metadata_parts.is_empty() {
                    output.push_str(&format!("({})\n", metadata_parts.join(", ")));
                }
            }

            if i < messages.len() - 1 {
                output.push_str("\n");
            }
        }

        Ok(output)
    }
}

/// JSON format handler
pub struct JsonHandler;

impl FormatHandler for JsonHandler {
    fn export_session_with_messages(
        &self,
        session: &ChatSession,
        messages: &[Message],
        options: &ExportOptions,
    ) -> Result<String> {
        let export_data = if options.include_metadata {
            serde_json::json!({
                "session": {
                    "id": session.id,
                    "title": session.title,
                    "created_at": session.created_at,
                    "updated_at": session.updated_at,
                    "model": session.model,
                    "system_prompt": session.system_prompt,
                    "model_config": session.model_config,
                    "total_tokens_used": if options.include_token_usage { Some(&session.total_tokens_used) } else { None },
                    "tags": session.tags,
                    "is_archived": session.is_archived,
                    "export_count": session.export_count,
                    "message_count": session.message_count,
                    "last_activity": session.last_activity
                },
                "messages": messages.iter().map(|msg| {
                    let mut message_data = serde_json::json!({
                        "id": msg.id,
                        "role": msg.role,
                        "content": msg.content,
                        "timestamp": msg.timestamp,
                        "edited_at": msg.edited_at,
                        "parent_id": msg.parent_id,
                        "children": msg.children
                    });

                    if options.include_token_usage {
                        message_data["token_usage"] = serde_json::to_value(&msg.token_usage)?;
                    }

                    if options.include_metadata {
                        message_data["metadata"] = serde_json::to_value(&msg.metadata)?;
                    }

                    Ok::<_, serde_json::Error>(message_data)
                }).collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| EnhancedError::unknown(format!("Failed to serialize message: {}", e)))?
            })
        } else {
            serde_json::json!({
                "title": session.title,
                "model": session.model,
                "messages": messages.iter().map(|msg| {
                    serde_json::json!({
                        "role": msg.role,
                        "content": msg.content,
                        "timestamp": if options.include_timestamps { Some(msg.timestamp) } else { None }
                    })
                }).collect::<Vec<_>>()
            })
        };

        if options.pretty_format {
            Ok(serde_json::to_string_pretty(&export_data)?)
        } else {
            Ok(serde_json::to_string(&export_data)?)
        }
    }

    fn export_messages(&self, messages: &[Message], options: &ExportOptions) -> Result<String> {
        let messages_data: Vec<_> = messages
            .iter()
            .map(|msg| {
                let mut message_data = serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                });

                if options.include_timestamps {
                    message_data["timestamp"] = serde_json::to_value(msg.timestamp)?;
                }

                if options.include_model_info && matches!(msg.role, MessageRole::Assistant) {
                    message_data["model"] = serde_json::to_value(&msg.metadata.model_used)?;
                }

                if options.include_token_usage {
                    message_data["token_usage"] = serde_json::to_value(&msg.token_usage)?;
                }

                if options.include_metadata {
                    message_data["metadata"] = serde_json::to_value(&msg.metadata)?;
                    message_data["edited_at"] = serde_json::to_value(&msg.edited_at)?;
                    message_data["parent_id"] = serde_json::to_value(&msg.parent_id)?;
                    message_data["children"] = serde_json::to_value(&msg.children)?;
                }

                Ok::<_, serde_json::Error>(message_data)
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| EnhancedError::unknown(format!("Failed to serialize message: {}", e)))?;

        if options.pretty_format {
            Ok(serde_json::to_string_pretty(&messages_data)?)
        } else {
            Ok(serde_json::to_string(&messages_data)?)
        }
    }
}

/// HTML format handler
pub struct HtmlHandler;

impl FormatHandler for HtmlHandler {
    fn export_session_with_messages(
        &self,
        session: &ChatSession,
        messages: &[Message],
        options: &ExportOptions,
    ) -> Result<String> {
        let mut output = String::new();

        // HTML header
        output.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        output.push_str("    <meta charset=\"UTF-8\">\n");
        output.push_str(
            "    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        );
        output.push_str(&format!(
            "    <title>{}</title>\n",
            html_escape(&session.title)
        ));
        output.push_str("    <style>\n");
        output.push_str(include_str!("../../assets/export.css"));
        output.push_str("    </style>\n");
        output.push_str("</head>\n<body>\n");

        // Session header
        output.push_str(&format!(
            "    <header>\n        <h1>{}</h1>\n",
            html_escape(&session.title)
        ));

        if options.include_metadata {
            output.push_str("        <div class=\"session-info\">\n");
            output.push_str(&format!(
                "            <p><strong>Created:</strong> {}</p>\n",
                session.created_at.format("%Y-%m-%d %H:%M:%S")
            ));
            output.push_str(&format!(
                "            <p><strong>Model:</strong> {}</p>\n",
                html_escape(&session.model)
            ));

            if let Some(ref system_prompt) = session.system_prompt {
                output.push_str(&format!(
                    "            <p><strong>System Prompt:</strong> {}</p>\n",
                    html_escape(system_prompt)
                ));
            }

            if options.include_token_usage {
                output.push_str(&format!(
                    "            <p><strong>Total Tokens:</strong> {}</p>\n",
                    session.total_tokens_used.total_tokens
                ));
            }

            if !session.tags.is_empty() {
                output.push_str(&format!(
                    "            <p><strong>Tags:</strong> {}</p>\n",
                    html_escape(&session.tags.join(", "))
                ));
            }

            output.push_str("        </div>\n");
        }

        output.push_str("    </header>\n\n");

        // Messages (passed as parameter - single source of truth)
        output.push_str("    <main class=\"conversation\">\n");
        let messages_html = self.export_messages(messages, options)?;
        output.push_str(&messages_html);
        output.push_str("    </main>\n");

        // HTML footer
        output.push_str("</body>\n</html>");

        Ok(output)
    }

    fn export_messages(&self, messages: &[Message], options: &ExportOptions) -> Result<String> {
        let mut output = String::new();

        for message in messages {
            let role_class = match message.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::System => "system",
            };

            let role_icon = match message.role {
                MessageRole::User => "👤",
                MessageRole::Assistant => "🤖",
                MessageRole::System => "⚙️",
            };

            output.push_str(&format!("        <div class=\"message {}\">\n", role_class));
            output.push_str("            <div class=\"message-header\">\n");
            output.push_str(&format!(
                "                <span class=\"role\">{} {}</span>\n",
                role_icon,
                role_class.to_uppercase()
            ));

            if options.include_timestamps {
                output.push_str(&format!(
                    "                <span class=\"timestamp\">{}</span>\n",
                    message.timestamp.format("%Y-%m-%d %H:%M:%S")
                ));
            }

            if options.include_model_info && matches!(message.role, MessageRole::Assistant) {
                output.push_str(&format!(
                    "                <span class=\"model\">{}</span>\n",
                    html_escape(&message.metadata.model_used)
                ));
            }

            output.push_str("            </div>\n");

            // Message content (convert markdown to HTML if needed)
            output.push_str("            <div class=\"message-content\">\n");
            let content_html = convert_markdown_to_html(&message.content);
            output.push_str(&format!("                {}\n", content_html));
            output.push_str("            </div>\n");

            // Additional metadata
            if options.include_metadata {
                let mut metadata_parts = Vec::new();

                if let Some(ref token_usage) = message.token_usage {
                    metadata_parts.push(format!("Tokens: {}", token_usage.total_tokens));
                }

                if message.metadata.response_time_ms > 0 {
                    metadata_parts.push(format!(
                        "Response time: {}ms",
                        message.metadata.response_time_ms
                    ));
                }

                if message.metadata.is_regenerated {
                    metadata_parts.push(format!(
                        "Regenerated {} times",
                        message.metadata.regeneration_count
                    ));
                }

                if message.edited_at.is_some() {
                    metadata_parts.push("Edited".to_string());
                }

                if !metadata_parts.is_empty() {
                    output.push_str("            <div class=\"message-metadata\">\n");
                    output.push_str(&format!(
                        "                <small>{}</small>\n",
                        html_escape(&metadata_parts.join(" • "))
                    ));
                    output.push_str("            </div>\n");
                }
            }

            output.push_str("        </div>\n");
        }

        Ok(output)
    }
}

/// Escape HTML special characters
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Convert markdown to HTML (basic implementation)
fn convert_markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{html, Parser};

    let parser = Parser::new(markdown);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}
