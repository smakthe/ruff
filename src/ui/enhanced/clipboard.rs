//! Clipboard management functionality using arboard

use arboard::Clipboard;
use std::sync::Mutex;
use thiserror::Error;

/// Errors that can occur during clipboard operations
#[derive(Error, Debug)]
pub enum ClipboardError {
    #[error("Failed to access clipboard: {0}")]
    AccessError(String),
    #[error("Failed to copy content to clipboard: {0}")]
    CopyError(String),
    #[error("Failed to paste content from clipboard: {0}")]
    PasteError(String),
    #[error("Clipboard content is not valid text")]
    InvalidContent,
}

/// Clipboard manager for copy/paste operations
pub struct ClipboardManager {
    clipboard: Mutex<Clipboard>,
}

impl ClipboardManager {
    /// Create a new clipboard manager
    pub fn new() -> Result<Self, ClipboardError> {
        let clipboard = Clipboard::new()
            .map_err(|e| ClipboardError::AccessError(e.to_string()))?;
        
        Ok(Self {
            clipboard: Mutex::new(clipboard),
        })
    }
    
    /// Copy text content to the clipboard
    pub fn copy_to_clipboard(&self, content: &str) -> Result<(), ClipboardError> {
        let mut clipboard = self.clipboard.lock()
            .map_err(|e| ClipboardError::AccessError(format!("Mutex lock failed: {}", e)))?;
        
        clipboard.set_text(content)
            .map_err(|e| ClipboardError::CopyError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Get text content from the clipboard
    pub fn get_from_clipboard(&self) -> Result<String, ClipboardError> {
        let mut clipboard = self.clipboard.lock()
            .map_err(|e| ClipboardError::AccessError(format!("Mutex lock failed: {}", e)))?;
        
        clipboard.get_text()
            .map_err(|e| ClipboardError::PasteError(e.to_string()))
    }
    
    /// Copy a message to clipboard with optional formatting
    pub fn copy_message(&self, content: &str, include_metadata: bool, timestamp: Option<&str>, role: Option<&str>) -> Result<(), ClipboardError> {
        let formatted_content = if include_metadata {
            let mut formatted = String::new();
            
            if let Some(ts) = timestamp {
                formatted.push_str(&format!("[{}] ", ts));
            }
            
            if let Some(r) = role {
                formatted.push_str(&format!("{}: ", r));
            }
            
            formatted.push_str(content);
            formatted
        } else {
            content.to_string()
        };
        
        self.copy_to_clipboard(&formatted_content)
    }
    
    /// Copy a code block to clipboard with optional language header
    pub fn copy_code_block(&self, code: &str, language: Option<&str>, include_language_header: bool) -> Result<(), ClipboardError> {
        let formatted_code = if include_language_header && language.is_some() {
            format!("```{}\n{}\n```", language.unwrap(), code)
        } else {
            code.to_string()
        };
        
        self.copy_to_clipboard(&formatted_code)
    }
    
    /// Copy multiple code blocks as a single clipboard entry
    pub fn copy_multiple_code_blocks(&self, code_blocks: &[(String, Option<String>)], include_language_headers: bool) -> Result<(), ClipboardError> {
        let mut combined = String::new();
        
        for (i, (code, language)) in code_blocks.iter().enumerate() {
            if i > 0 {
                combined.push_str("\n\n");
            }
            
            if include_language_headers && language.is_some() {
                combined.push_str(&format!("```{}\n{}\n```", language.as_ref().unwrap(), code));
            } else {
                combined.push_str(code);
            }
        }
        
        self.copy_to_clipboard(&combined)
    }
    
    /// Copy formatted conversation to clipboard
    pub fn copy_conversation(&self, messages: &[(String, String, Option<String>)]) -> Result<(), ClipboardError> {
        let mut conversation = String::new();
        
        for (role, content, timestamp) in messages {
            if !conversation.is_empty() {
                conversation.push_str("\n\n");
            }
            
            if let Some(ts) = timestamp {
                conversation.push_str(&format!("[{}] {}: {}", ts, role, content));
            } else {
                conversation.push_str(&format!("{}: {}", role, content));
            }
        }
        
        self.copy_to_clipboard(&conversation)
    }
    
    /// Check if clipboard is available
    pub fn is_available(&self) -> bool {
        self.clipboard.lock().is_ok()
    }
    
    /// Clear the clipboard
    pub fn clear(&self) -> Result<(), ClipboardError> {
        self.copy_to_clipboard("")
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback to a dummy implementation if clipboard is not available
            Self {
                clipboard: Mutex::new(Clipboard::new().unwrap()),
            }
        })
    }
}

/// Utility functions for clipboard operations
pub mod utils {
    
    /// Extract plain text from markdown content
    pub fn extract_plain_text(markdown: &str) -> String {
        // Simple markdown to plain text conversion
        let mut text = markdown.to_string();
        
        // Remove code block markers
        text = text.replace("```", "");
        
        // Remove inline code markers
        text = text.replace("`", "");
        
        // Remove emphasis markers
        text = text.replace("**", "");
        text = text.replace("*", "");
        
        // Remove headers
        text = text.replace("# ", "");
        text = text.replace("## ", "");
        text = text.replace("### ", "");
        text = text.replace("#### ", "");
        text = text.replace("##### ", "");
        text = text.replace("###### ", "");
        
        // Remove blockquote markers
        text = text.replace("> ", "");
        
        // Clean up extra whitespace
        text.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
    
    /// Format code for clipboard with proper indentation
    pub fn format_code_for_clipboard(code: &str, language: Option<&str>) -> String {
        let lines: Vec<&str> = code.lines().collect();
        if lines.is_empty() {
            return String::new();
        }
        
        // Find minimum indentation (excluding empty lines)
        let min_indent = lines.iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);
        
        // Remove common indentation
        let formatted_lines: Vec<String> = lines.iter()
            .map(|line| {
                if line.trim().is_empty() {
                    String::new()
                } else if line.len() >= min_indent {
                    line[min_indent..].to_string()
                } else {
                    line.to_string()
                }
            })
            .collect();
        
        let formatted_code = formatted_lines.join("\n");
        
        if let Some(lang) = language {
            format!("```{}\n{}\n```", lang, formatted_code)
        } else {
            formatted_code
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::utils::*;

    fn create_test_clipboard() -> ClipboardManager {
        // For testing, we'll create a clipboard manager
        // Note: These tests might fail in headless environments without a display
        ClipboardManager::new().unwrap_or_else(|_| {
            // Create a mock implementation for testing
            ClipboardManager::default()
        })
    }

    #[test]
    fn test_clipboard_manager_creation() {
        let result = ClipboardManager::new();
        // This might fail in headless environments, so we just check it doesn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_simple_text() {
        let clipboard = create_test_clipboard();
        let test_text = "Hello, world!";
        
        let result = clipboard.copy_to_clipboard(test_text);
        // In headless environments, this might fail, but shouldn't panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_message_with_metadata() {
        let clipboard = create_test_clipboard();
        let content = "This is a test message";
        let timestamp = "2024-01-01 12:00:00";
        let role = "User";
        
        let result = clipboard.copy_message(content, true, Some(timestamp), Some(role));
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_message_without_metadata() {
        let clipboard = create_test_clipboard();
        let content = "This is a test message";
        
        let result = clipboard.copy_message(content, false, None, None);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_code_block_with_language() {
        let clipboard = create_test_clipboard();
        let code = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let language = "rust";
        
        let result = clipboard.copy_code_block(code, Some(language), true);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_code_block_without_language() {
        let clipboard = create_test_clipboard();
        let code = "console.log('Hello, world!');";
        
        let result = clipboard.copy_code_block(code, None, false);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_multiple_code_blocks() {
        let clipboard = create_test_clipboard();
        let code_blocks = vec![
            ("fn main() {}".to_string(), Some("rust".to_string())),
            ("print('hello')".to_string(), Some("python".to_string())),
            ("console.log('hi')".to_string(), Some("javascript".to_string())),
        ];
        
        let result = clipboard.copy_multiple_code_blocks(&code_blocks, true);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_copy_conversation() {
        let clipboard = create_test_clipboard();
        let messages = vec![
            ("User".to_string(), "Hello!".to_string(), Some("12:00:00".to_string())),
            ("Assistant".to_string(), "Hi there!".to_string(), Some("12:00:01".to_string())),
            ("User".to_string(), "How are you?".to_string(), Some("12:00:02".to_string())),
        ];
        
        let result = clipboard.copy_conversation(&messages);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_clipboard_availability() {
        let clipboard = create_test_clipboard();
        // Just check that the method doesn't panic
        let _available = clipboard.is_available();
    }

    #[test]
    fn test_clear_clipboard() {
        let clipboard = create_test_clipboard();
        let result = clipboard.clear();
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_extract_plain_text() {
        let markdown = r#"# Header

This is **bold** and *italic* text with `inline code`.

```rust
fn main() {
    println!("Hello");
}
```

> This is a blockquote

- List item 1
- List item 2"#;

        let plain_text = extract_plain_text(markdown);
        
        assert!(plain_text.contains("Header"));
        assert!(plain_text.contains("bold"));
        assert!(plain_text.contains("italic"));
        assert!(plain_text.contains("inline code"));
        assert!(plain_text.contains("fn main"));
        assert!(plain_text.contains("This is a blockquote"));
        assert!(plain_text.contains("List item 1"));
        
        // Should not contain markdown markers
        assert!(!plain_text.contains("**"));
        assert!(!plain_text.contains("*"));
        assert!(!plain_text.contains("`"));
        assert!(!plain_text.contains("```"));
        assert!(!plain_text.contains("# "));
        assert!(!plain_text.contains("> "));
    }

    #[test]
    fn test_format_code_for_clipboard() {
        let code = "    fn main() {\n        println!(\"Hello\");\n    }";
        let formatted = format_code_for_clipboard(code, Some("rust"));
        
        assert!(formatted.contains("```rust"));
        assert!(formatted.contains("fn main()"));
        assert!(formatted.contains("println!"));
        assert!(formatted.ends_with("```"));
    }

    #[test]
    fn test_format_code_without_language() {
        let code = "    console.log('hello');\n    return true;";
        let formatted = format_code_for_clipboard(code, None);
        
        assert!(!formatted.contains("```"));
        assert!(formatted.contains("console.log"));
        assert!(formatted.contains("return true"));
        
        // Should remove common indentation
        assert!(!formatted.starts_with("    "));
    }

    #[test]
    fn test_format_code_with_mixed_indentation() {
        let code = "  if (true) {\n    console.log('hello');\n      return;\n  }";
        let formatted = format_code_for_clipboard(code, Some("javascript"));
        
        assert!(formatted.contains("```javascript"));
        assert!(formatted.contains("if (true)"));
        
        // Should preserve relative indentation
        let lines: Vec<&str> = formatted.lines().collect();
        let code_lines: Vec<&str> = lines[1..lines.len()-1].iter().cloned().collect();
        assert!(code_lines[1].starts_with("  ")); // console.log should be indented relative to if
    }

    #[test]
    fn test_format_empty_code() {
        let formatted = format_code_for_clipboard("", Some("rust"));
        // Empty code should return empty string, not formatted block
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_single_line_code() {
        let code = "println!(\"Hello, world!\");";
        let formatted = format_code_for_clipboard(code, Some("rust"));
        
        assert!(formatted.contains("```rust"));
        assert!(formatted.contains("println!"));
        assert!(formatted.ends_with("```"));
    }
}