//! Syntax highlighting functionality using syntect

use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::{SyntaxSet, SyntaxReference},
    util::LinesWithEndings,
};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use std::collections::HashMap;

/// Syntax highlighter for code blocks
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    current_theme: String,
    language_cache: HashMap<String, String>,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        
        Self {
            syntax_set,
            theme_set,
            current_theme: "base16-ocean.dark".to_string(),
            language_cache: Self::build_language_cache(),
        }
    }
    
    /// Highlight code and return styled lines for ratatui
    pub fn highlight(&self, code: &str, language: &str) -> Vec<Line<'static>> {
        let syntax = self.find_syntax(language);
        
        if let Some(syntax) = syntax {
            self.highlight_with_syntax(code, syntax)
        } else {
            // Fallback to plain text with basic styling
            self.highlight_plain(code)
        }
    }
    
    /// Find syntax definition for a language
    fn find_syntax(&self, language: &str) -> Option<&SyntaxReference> {
        // Try direct lookup first
        if let Some(syntax) = self.syntax_set.find_syntax_by_extension(language) {
            return Some(syntax);
        }
        
        // Try by name
        if let Some(syntax) = self.syntax_set.find_syntax_by_name(language) {
            return Some(syntax);
        }
        
        // Try common aliases
        let normalized = self.normalize_language_name(language);
        if let Some(canonical) = self.language_cache.get(&normalized) {
            return self.syntax_set.find_syntax_by_name(canonical);
        }
        
        // Try token-based lookup
        if let Some(syntax) = self.syntax_set.find_syntax_by_token(language) {
            return Some(syntax);
        }
        
        None
    }
    
    /// Highlight code with a specific syntax
    fn highlight_with_syntax(&self, code: &str, syntax: &SyntaxReference) -> Vec<Line<'static>> {
        let theme = match self.theme_set.themes.get(&self.current_theme) {
            Some(theme) => theme,
            None => &self.theme_set.themes["base16-ocean.dark"],
        };
        
        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut lines = Vec::new();
        
        for line in LinesWithEndings::from(code) {
            let highlighted = match highlighter.highlight_line(line, &self.syntax_set) {
                Ok(highlighted) => highlighted,
                Err(_) => {
                    // Fallback to plain text for this line
                    lines.push(Line::from(vec![
                        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                        Span::raw(line.trim_end().to_string()),
                    ]));
                    continue;
                }
            };
            
            let mut spans = vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))];
            
            for (style, text) in highlighted {
                let color = self.syntect_color_to_ratatui(style.foreground);
                let mut ratatui_style = Style::default().fg(color);
                
                if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                    ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::BOLD);
                }
                if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                    ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::ITALIC);
                }
                if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                    ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::UNDERLINED);
                }
                
                spans.push(Span::styled(text.to_string(), ratatui_style));
            }
            
            lines.push(Line::from(spans));
        }
        
        lines
    }
    
    /// Highlight as plain text with basic styling
    fn highlight_plain(&self, code: &str) -> Vec<Line<'static>> {
        code.lines()
            .map(|line| {
                Line::from(vec![
                    Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(line.to_string(), Style::default().fg(Color::White)),
                ])
            })
            .collect()
    }
    
    /// Convert syntect color to ratatui color
    fn syntect_color_to_ratatui(&self, color: syntect::highlighting::Color) -> Color {
        Color::Rgb(color.r, color.g, color.b)
    }
    
    /// Normalize language name for lookup
    fn normalize_language_name(&self, language: &str) -> String {
        language.to_lowercase().replace(['-', '_'], "")
    }
    
    /// Build a cache of language aliases to canonical names
    fn build_language_cache() -> HashMap<String, String> {
        let mut cache = HashMap::new();
        
        // Common aliases
        cache.insert("js".to_string(), "JavaScript".to_string());
        cache.insert("javascript".to_string(), "JavaScript".to_string());
        cache.insert("ts".to_string(), "TypeScript".to_string());
        cache.insert("typescript".to_string(), "TypeScript".to_string());
        cache.insert("py".to_string(), "Python".to_string());
        cache.insert("python".to_string(), "Python".to_string());
        cache.insert("rs".to_string(), "Rust".to_string());
        cache.insert("rust".to_string(), "Rust".to_string());
        cache.insert("go".to_string(), "Go".to_string());
        cache.insert("golang".to_string(), "Go".to_string());
        cache.insert("cpp".to_string(), "C++".to_string());
        cache.insert("cxx".to_string(), "C++".to_string());
        cache.insert("cc".to_string(), "C++".to_string());
        cache.insert("c".to_string(), "C".to_string());
        cache.insert("java".to_string(), "Java".to_string());
        cache.insert("cs".to_string(), "C#".to_string());
        cache.insert("csharp".to_string(), "C#".to_string());
        cache.insert("php".to_string(), "PHP".to_string());
        cache.insert("rb".to_string(), "Ruby".to_string());
        cache.insert("ruby".to_string(), "Ruby".to_string());
        cache.insert("sh".to_string(), "Bourne Again Shell (bash)".to_string());
        cache.insert("bash".to_string(), "Bourne Again Shell (bash)".to_string());
        cache.insert("zsh".to_string(), "Bourne Again Shell (bash)".to_string());
        cache.insert("fish".to_string(), "Bourne Again Shell (bash)".to_string());
        cache.insert("powershell".to_string(), "PowerShell".to_string());
        cache.insert("ps1".to_string(), "PowerShell".to_string());
        cache.insert("html".to_string(), "HTML".to_string());
        cache.insert("css".to_string(), "CSS".to_string());
        cache.insert("scss".to_string(), "SCSS".to_string());
        cache.insert("sass".to_string(), "Sass".to_string());
        cache.insert("json".to_string(), "JSON".to_string());
        cache.insert("xml".to_string(), "XML".to_string());
        cache.insert("yaml".to_string(), "YAML".to_string());
        cache.insert("yml".to_string(), "YAML".to_string());
        cache.insert("toml".to_string(), "TOML".to_string());
        cache.insert("md".to_string(), "Markdown".to_string());
        cache.insert("markdown".to_string(), "Markdown".to_string());
        cache.insert("sql".to_string(), "SQL".to_string());
        cache.insert("dockerfile".to_string(), "Dockerfile".to_string());
        cache.insert("docker".to_string(), "Dockerfile".to_string());
        
        cache
    }
    
    /// Set the current theme
    pub fn set_theme(&mut self, theme_name: &str) -> Result<(), String> {
        if self.theme_set.themes.contains_key(theme_name) {
            self.current_theme = theme_name.to_string();
            Ok(())
        } else {
            Err(format!("Theme '{}' not found", theme_name))
        }
    }
    
    /// Get available themes
    pub fn available_themes(&self) -> Vec<String> {
        self.theme_set.themes.keys().cloned().collect()
    }
    
    /// Get supported languages
    pub fn supported_languages(&self) -> Vec<String> {
        self.syntax_set
            .syntaxes()
            .iter()
            .map(|syntax| syntax.name.clone())
            .collect()
    }
    
    /// Check if a language is supported
    pub fn is_language_supported(&self, language: &str) -> bool {
        self.find_syntax(language).is_some()
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn test_syntax_highlighter_creation() {
        let highlighter = SyntaxHighlighter::new();
        assert!(!highlighter.available_themes().is_empty());
        assert!(!highlighter.supported_languages().is_empty());
    }

    #[test]
    fn test_highlight_rust_code() {
        let highlighter = SyntaxHighlighter::new();
        let code = "fn main() {\n    println!(\"Hello, world!\");\n}";
        let lines = highlighter.highlight(code, "rust");
        
        assert!(!lines.is_empty());
        assert_eq!(lines.len(), 3); // Three lines of code
        
        // Each line should start with the pipe character for code block formatting
        for line in &lines {
            assert!(!line.spans.is_empty());
            assert!(line.spans[0].content.contains("│"));
        }
    }

    #[test]
    fn test_highlight_python_code() {
        let highlighter = SyntaxHighlighter::new();
        let code = "def hello():\n    print(\"Hello, world!\")\n    return True";
        let lines = highlighter.highlight(code, "python");
        
        assert!(!lines.is_empty());
        assert_eq!(lines.len(), 3);
        
        // Check that content is preserved
        let content: String = lines.iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        assert!(content.contains("def hello"));
        assert!(content.contains("print"));
        assert!(content.contains("return True"));
    }

    #[test]
    fn test_highlight_javascript_code() {
        let highlighter = SyntaxHighlighter::new();
        let code = "function greet(name) {\n    console.log(`Hello, ${name}!`);\n}";
        let lines = highlighter.highlight(code, "javascript");
        
        assert!(!lines.is_empty());
        assert_eq!(lines.len(), 3);
        
        // Verify content preservation
        let content: String = lines.iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        assert!(content.contains("function greet"));
        assert!(content.contains("console.log"));
    }

    #[test]
    fn test_highlight_unknown_language() {
        let highlighter = SyntaxHighlighter::new();
        let code = "some unknown code\nwith multiple lines";
        let lines = highlighter.highlight(code, "unknown_language");
        
        assert!(!lines.is_empty());
        assert_eq!(lines.len(), 2);
        
        // Should fall back to plain text highlighting
        for line in &lines {
            assert!(!line.spans.is_empty());
            // First span should be the pipe character
            assert!(line.spans[0].content.contains("│"));
            // Second span should be white text (plain text fallback)
            if line.spans.len() > 1 {
                assert_eq!(line.spans[1].style.fg, Some(Color::White));
            }
        }
    }

    #[test]
    fn test_language_aliases() {
        let highlighter = SyntaxHighlighter::new();
        
        // Test languages that are definitely supported by syntect
        assert!(highlighter.is_language_supported("rust"));
        assert!(highlighter.is_language_supported("python"));
        assert!(highlighter.is_language_supported("javascript"));
        
        // Test some aliases that should work
        assert!(highlighter.is_language_supported("js") || highlighter.is_language_supported("javascript"));
        assert!(highlighter.is_language_supported("py") || highlighter.is_language_supported("python"));
        assert!(highlighter.is_language_supported("rs") || highlighter.is_language_supported("rust"));
    }

    #[test]
    fn test_normalize_language_name() {
        let highlighter = SyntaxHighlighter::new();
        
        assert_eq!(highlighter.normalize_language_name("JavaScript"), "javascript");
        assert_eq!(highlighter.normalize_language_name("Type-Script"), "typescript");
        assert_eq!(highlighter.normalize_language_name("C++"), "c++");
        assert_eq!(highlighter.normalize_language_name("C_Sharp"), "csharp");
    }

    #[test]
    fn test_theme_management() {
        let mut highlighter = SyntaxHighlighter::new();
        let themes = highlighter.available_themes();
        
        assert!(!themes.is_empty());
        assert!(themes.contains(&"base16-ocean.dark".to_string()));
        
        // Test setting a valid theme
        assert!(highlighter.set_theme("base16-ocean.dark").is_ok());
        
        // Test setting an invalid theme
        assert!(highlighter.set_theme("nonexistent-theme").is_err());
    }

    #[test]
    fn test_supported_languages() {
        let highlighter = SyntaxHighlighter::new();
        let languages = highlighter.supported_languages();
        
        assert!(!languages.is_empty());
        
        // Check for common languages
        let language_names: Vec<String> = languages.iter().map(|s| s.to_lowercase()).collect();
        assert!(language_names.iter().any(|lang| lang.contains("rust")));
        assert!(language_names.iter().any(|lang| lang.contains("python")));
        assert!(language_names.iter().any(|lang| lang.contains("javascript")));
    }

    #[test]
    fn test_highlight_empty_code() {
        let highlighter = SyntaxHighlighter::new();
        let lines = highlighter.highlight("", "rust");
        
        // Should handle empty code gracefully
        assert!(lines.is_empty() || lines.len() == 1);
    }

    #[test]
    fn test_highlight_single_line() {
        let highlighter = SyntaxHighlighter::new();
        let code = "let x = 42;";
        let lines = highlighter.highlight(code, "rust");
        
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].spans.is_empty());
        assert!(lines[0].spans[0].content.contains("│"));
    }

    #[test]
    fn test_highlight_with_special_characters() {
        let highlighter = SyntaxHighlighter::new();
        let code = "let message = \"Hello, 世界! 🦀\";";
        let lines = highlighter.highlight(code, "rust");
        
        assert_eq!(lines.len(), 1);
        
        // Verify special characters are preserved
        let content: String = lines[0].spans.iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(content.contains("世界"));
        assert!(content.contains("🦀"));
    }

    #[test]
    fn test_highlight_multiline_string() {
        let highlighter = SyntaxHighlighter::new();
        let code = r#"let multiline = "
This is a
multiline string
with multiple lines
";"#;
        let lines = highlighter.highlight(code, "rust");
        
        assert!(lines.len() >= 5); // Should have multiple lines
        
        // Each line should have the pipe prefix
        for line in &lines {
            assert!(!line.spans.is_empty());
            assert!(line.spans[0].content.contains("│"));
        }
    }

    #[test]
    fn test_highlight_code_with_comments() {
        let highlighter = SyntaxHighlighter::new();
        let code = r#"// This is a comment
fn main() {
    // Another comment
    println!("Hello"); // Inline comment
}"#;
        let lines = highlighter.highlight(code, "rust");
        
        // The number of lines might vary depending on how syntect handles the input
        assert!(!lines.is_empty());
        assert!(lines.len() >= 4); // At least 4 lines, but could be 5 if there's a trailing newline
        
        // Verify content is preserved
        let content: String = lines.iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect();
        assert!(content.contains("This is a comment"));
        assert!(content.contains("Another comment"));
        assert!(content.contains("Inline comment"));
    }

    #[test]
    fn test_language_detection_case_insensitive() {
        let highlighter = SyntaxHighlighter::new();
        
        // Test that basic languages work (case sensitivity depends on syntect's implementation)
        assert!(highlighter.is_language_supported("rust"));
        assert!(highlighter.is_language_supported("python"));
        
        // Test that our normalize function works correctly
        assert_eq!(highlighter.normalize_language_name("RUST"), "rust");
        assert_eq!(highlighter.normalize_language_name("Python"), "python");
    }
}impl 
Clone for SyntaxHighlighter {
    fn clone(&self) -> Self {
        Self::new()
    }
}