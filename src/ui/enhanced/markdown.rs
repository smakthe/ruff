//! Markdown rendering functionality for terminal display

use crate::ui::enhanced::SyntaxHighlighter;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

/// Markdown renderer for rich text display in terminal
#[derive(Clone)]
pub struct MarkdownRenderer {
    syntax_highlighter: SyntaxHighlighter,
}

/// Rendered markdown content with styling information
#[derive(Debug, Clone)]
pub struct RenderedMarkdown {
    pub text: Text<'static>,
    pub code_blocks: Vec<CodeBlock>,
}

/// Information about a code block for copy functionality
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub content: String,
    pub language: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            syntax_highlighter: SyntaxHighlighter::new(),
        }
    }

    /// Render markdown text to ratatui Text with styling
    pub fn render(&self, markdown: &str) -> RenderedMarkdown {
        let parser = Parser::new(markdown);
        let mut lines = Vec::new();
        let mut code_blocks = Vec::new();
        let mut current_line = Vec::new();
        let mut in_code_block = false;
        let mut code_block_content = String::new();
        let mut code_block_language = None;
        let mut code_block_start_line = 0;
        let mut line_number = 0;

        for event in parser {
            match event {
                Event::Start(tag) => {
                    match tag {
                        Tag::Heading { level, .. } => {
                            let level_num = match level {
                                HeadingLevel::H1 => 1,
                                HeadingLevel::H2 => 2,
                                HeadingLevel::H3 => 3,
                                HeadingLevel::H4 => 4,
                                HeadingLevel::H5 => 5,
                                HeadingLevel::H6 => 6,
                            };
                            let style = match level_num {
                                1 => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                                2 => Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                                3 => Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD),
                                4 => Style::default()
                                    .fg(Color::Blue)
                                    .add_modifier(Modifier::BOLD),
                                5 => Style::default()
                                    .fg(Color::Magenta)
                                    .add_modifier(Modifier::BOLD),
                                _ => Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            };
                            current_line.push(Span::styled("#".repeat(level_num) + " ", style));
                        }
                        Tag::Emphasis => {
                            // Will be handled in text events
                        }
                        Tag::Strong => {
                            // Will be handled in text events
                        }
                        Tag::CodeBlock(kind) => {
                            in_code_block = true;
                            code_block_start_line = line_number;
                            code_block_content.clear();
                            code_block_language = match kind {
                                CodeBlockKind::Fenced(lang) => {
                                    if lang.is_empty() {
                                        None
                                    } else {
                                        Some(lang.to_string())
                                    }
                                }
                                CodeBlockKind::Indented => None,
                            };
                        }
                        Tag::List(_) => {
                            // Add spacing before lists
                            if !current_line.is_empty() {
                                lines.push(Line::from(current_line.clone()));
                                current_line.clear();
                                line_number += 1;
                            }
                        }
                        Tag::Item => {
                            current_line
                                .push(Span::styled("• ", Style::default().fg(Color::Yellow)));
                        }
                        Tag::BlockQuote(_) => {
                            current_line.push(Span::styled("│ ", Style::default().fg(Color::Blue)));
                        }
                        _ => {}
                    }
                }
                Event::End(tag_end) => {
                    match tag_end {
                        TagEnd::Heading(_) => {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                            line_number += 1;
                            // Add spacing after headers
                            lines.push(Line::from(vec![Span::raw("")]));
                            line_number += 1;
                        }
                        TagEnd::CodeBlock => {
                            in_code_block = false;

                            // Store code block information
                            code_blocks.push(CodeBlock {
                                content: code_block_content.clone(),
                                language: code_block_language.clone(),
                                start_line: code_block_start_line,
                                end_line: line_number,
                            });

                            // Render code block with syntax highlighting
                            let highlighted_lines = if let Some(ref lang) = code_block_language {
                                self.syntax_highlighter.highlight(&code_block_content, lang)
                            } else {
                                self.render_plain_code(&code_block_content)
                            };

                            // Add code block header
                            if let Some(ref lang) = code_block_language {
                                lines.push(Line::from(vec![
                                    Span::styled("┌─ ", Style::default().fg(Color::DarkGray)),
                                    Span::styled(lang.clone(), Style::default().fg(Color::Cyan)),
                                    Span::styled(
                                        " ─".repeat(20),
                                        Style::default().fg(Color::DarkGray),
                                    ),
                                ]));
                                line_number += 1;
                            }

                            // Add highlighted code lines
                            for line in highlighted_lines {
                                lines.push(line);
                                line_number += 1;
                            }

                            // Add code block footer
                            lines.push(Line::from(vec![Span::styled(
                                "└─".repeat(25),
                                Style::default().fg(Color::DarkGray),
                            )]));
                            line_number += 1;
                        }
                        TagEnd::Paragraph => {
                            if !current_line.is_empty() {
                                lines.push(Line::from(current_line.clone()));
                                current_line.clear();
                                line_number += 1;
                            }
                            // Add spacing after paragraphs
                            lines.push(Line::from(vec![Span::raw("")]));
                            line_number += 1;
                        }
                        TagEnd::Item => {
                            lines.push(Line::from(current_line.clone()));
                            current_line.clear();
                            line_number += 1;
                        }
                        TagEnd::List(_) => {
                            // Add spacing after lists
                            lines.push(Line::from(vec![Span::raw("")]));
                            line_number += 1;
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    if in_code_block {
                        code_block_content.push_str(&text);
                    } else {
                        current_line.push(Span::raw(text.to_string()));
                    }
                }
                Event::Code(code) => {
                    current_line.push(Span::styled(
                        format!("`{}`", code),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }
                Event::SoftBreak | Event::HardBreak => {
                    if !in_code_block {
                        lines.push(Line::from(current_line.clone()));
                        current_line.clear();
                        line_number += 1;
                    } else {
                        code_block_content.push('\n');
                    }
                }
                _ => {}
            }
        }

        // Add any remaining content
        if !current_line.is_empty() {
            lines.push(Line::from(current_line));
        }

        RenderedMarkdown {
            text: Text::from(lines),
            code_blocks,
        }
    }

    /// Render plain code without syntax highlighting
    fn render_plain_code(&self, code: &str) -> Vec<Line<'static>> {
        code.lines()
            .map(|line| {
                Line::from(vec![
                    Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(line.to_string(), Style::default().fg(Color::White)),
                ])
            })
            .collect()
    }

    /// Extract plain text from markdown (useful for search)
    pub fn extract_text(&self, markdown: &str) -> String {
        let parser = Parser::new(markdown);
        let mut text = String::new();

        for event in parser {
            match event {
                Event::Text(t) | Event::Code(t) => {
                    text.push_str(&t);
                }
                Event::SoftBreak | Event::HardBreak => {
                    text.push(' ');
                }
                _ => {}
            }
        }

        text
    }

    /// Check if markdown contains code blocks
    pub fn has_code_blocks(&self, markdown: &str) -> bool {
        let parser = Parser::new(markdown);

        for event in parser {
            if matches!(event, Event::Start(Tag::CodeBlock(_))) {
                return true;
            }
        }

        false
    }

    /// Extract all code blocks from markdown
    pub fn extract_code_blocks(&self, markdown: &str) -> Vec<CodeBlock> {
        self.render(markdown).code_blocks
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    #[test]
    fn test_markdown_renderer_creation() {
        let renderer = MarkdownRenderer::new();
        // Test that the renderer can be created successfully
        let result = renderer.render("Hello, world!");
        assert!(!result.text.lines.is_empty());
    }

    #[test]
    fn test_render_plain_text() {
        let renderer = MarkdownRenderer::new();
        let result = renderer.render("Hello, world!");

        assert!(!result.text.lines.is_empty());
        // Should have content plus spacing
        assert!(result.text.lines.len() >= 1);
        assert!(result.code_blocks.is_empty());
    }

    #[test]
    fn test_render_headers() {
        let renderer = MarkdownRenderer::new();
        let markdown = "# Header 1\n## Header 2\n### Header 3";
        let result = renderer.render(markdown);

        // Should have headers with proper styling
        assert!(result.text.lines.len() >= 3);
        assert!(result.code_blocks.is_empty());

        // Check that first line contains header content
        let first_line = &result.text.lines[0];
        assert!(!first_line.spans.is_empty());
    }

    #[test]
    fn test_render_emphasis() {
        let renderer = MarkdownRenderer::new();
        let markdown = "This is *italic* and **bold** text.";
        let result = renderer.render(markdown);

        assert!(!result.text.lines.is_empty());
        assert!(result.code_blocks.is_empty());
    }

    #[test]
    fn test_render_inline_code() {
        let renderer = MarkdownRenderer::new();
        let markdown = "Here is some `inline code` in text.";
        let result = renderer.render(markdown);

        assert!(!result.text.lines.is_empty());
        assert!(result.code_blocks.is_empty());

        // Check that inline code is styled differently
        let line = &result.text.lines[0];
        let has_code_style = line.spans.iter().any(|span| {
            span.style.fg == Some(Color::Yellow)
                && span.style.add_modifier.contains(Modifier::ITALIC)
        });
        assert!(has_code_style);
    }

    #[test]
    fn test_render_code_block() {
        let renderer = MarkdownRenderer::new();
        let markdown = "```rust\nfn main() {\n    println!(\"Hello, world!\");\n}\n```";
        let result = renderer.render(markdown);

        assert!(!result.text.lines.is_empty());
        assert_eq!(result.code_blocks.len(), 1);

        let code_block = &result.code_blocks[0];
        assert_eq!(code_block.language, Some("rust".to_string()));
        assert!(code_block.content.contains("fn main()"));
        assert!(code_block.content.contains("println!"));
    }

    #[test]
    fn test_render_code_block_without_language() {
        let renderer = MarkdownRenderer::new();
        let markdown = "```\nsome code\nwithout language\n```";
        let result = renderer.render(markdown);

        assert!(!result.text.lines.is_empty());
        assert_eq!(result.code_blocks.len(), 1);

        let code_block = &result.code_blocks[0];
        assert_eq!(code_block.language, None);
        assert!(code_block.content.contains("some code"));
    }

    #[test]
    fn test_render_list() {
        let renderer = MarkdownRenderer::new();
        let markdown = "- Item 1\n- Item 2\n- Item 3";
        let result = renderer.render(markdown);

        assert!(!result.text.lines.is_empty());
        assert!(result.code_blocks.is_empty());

        // Should have bullet points
        let has_bullets = result
            .text
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content.contains("•")));
        assert!(has_bullets);
    }

    #[test]
    fn test_render_blockquote() {
        let renderer = MarkdownRenderer::new();
        let markdown = "> This is a blockquote\n> with multiple lines";
        let result = renderer.render(markdown);

        assert!(!result.text.lines.is_empty());
        assert!(result.code_blocks.is_empty());

        // Should have blockquote indicators
        let has_quote_indicator = result
            .text
            .lines
            .iter()
            .any(|line| line.spans.iter().any(|span| span.content.contains("│")));
        assert!(has_quote_indicator);
    }

    #[test]
    fn test_extract_text() {
        let renderer = MarkdownRenderer::new();
        let markdown = "# Header\n\nSome **bold** text with `code`.\n\n```rust\nfn test() {}\n```";
        let text = renderer.extract_text(markdown);

        assert!(text.contains("Header"));
        assert!(text.contains("Some"));
        assert!(text.contains("bold"));
        assert!(text.contains("text"));
        assert!(text.contains("code"));
        // Code block content should also be included
        assert!(text.contains("fn test() {}"));
    }

    #[test]
    fn test_has_code_blocks() {
        let renderer = MarkdownRenderer::new();

        let markdown_with_code = "Some text\n```rust\ncode here\n```";
        assert!(renderer.has_code_blocks(markdown_with_code));

        let markdown_without_code = "Just some text with `inline code`";
        assert!(!renderer.has_code_blocks(markdown_without_code));
    }

    #[test]
    fn test_extract_code_blocks() {
        let renderer = MarkdownRenderer::new();
        let markdown =
            "Text\n```rust\nfn main() {}\n```\nMore text\n```python\nprint('hello')\n```";
        let code_blocks = renderer.extract_code_blocks(markdown);

        assert_eq!(code_blocks.len(), 2);

        assert_eq!(code_blocks[0].language, Some("rust".to_string()));
        assert!(code_blocks[0].content.contains("fn main()"));

        assert_eq!(code_blocks[1].language, Some("python".to_string()));
        assert!(code_blocks[1].content.contains("print('hello')"));
    }

    #[test]
    fn test_complex_markdown() {
        let renderer = MarkdownRenderer::new();
        let markdown = r#"
# Main Title

This is a paragraph with **bold** and *italic* text, plus `inline code`.

## Subsection

> This is a blockquote
> with multiple lines

### Code Example

```rust
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```

### List of Items

- First item
- Second item with `code`
- Third item

That's all!
"#;

        let result = renderer.render(markdown);

        // Should have content
        assert!(!result.text.lines.is_empty());

        // Should have one code block
        assert_eq!(result.code_blocks.len(), 1);
        assert_eq!(result.code_blocks[0].language, Some("rust".to_string()));
        assert!(result.code_blocks[0].content.contains("fibonacci"));

        // Extract text should work
        let text = renderer.extract_text(markdown);
        assert!(text.contains("Main Title"));
        assert!(text.contains("fibonacci"));
        assert!(text.contains("First item"));
    }
}
