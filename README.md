# 🦀 Ruff - AI Chat Application for your Terminal

A blazingly fast, terminal-based AI chat application built in Rust with support for multiple AI models and providers.

## Features

- 🚀 **Multiple AI Models**: Support for OpenAI, Anthropic, Cohere, Together AI, Groq, and Hugging Face
- 💬 **Real-time Chat**: Interactive terminal-based chat interface
- 📊 **Token Tracking**: Detailed input/output token usage monitoring
- 🎨 **Rust-themed UI**: Beautiful terminal interface with Rust-inspired colors
- ⚡ **High Performance**: Built with async Rust for maximum performance
- 🔧 **Easy Configuration**: Simple TOML-based configuration management
- 📝 **Chat History**: Persistent chat sessions with timestamps
- 🔐 **Secure**: API keys stored locally and never transmitted

## Installation

### From Source

```bash
git clone https://github.com/yourusername/ruff
cd ruff
cargo install --path .
```

### From Crates.io

```bash
cargo install ruff
```

## Quick Start

1. **Initialize configuration**:
   ```bash
   ruff --init
   ```

2. **Configure your API keys**: Edit the configuration file with your API keys
   ```bash
   ruff --show-config  # Shows config file location
   ```

3. **Start chatting**:
   ```bash
   ruff
   ```

## Supported AI Models

| Provider | Models | API Key Format | Free Tier |
|----------|---------|----------------|-----------|
| **OpenAI** | GPT-4, GPT-3.5 Turbo | `sk-...` | ❌ |
| **Anthropic** | Claude 3 Sonnet | `sk-ant-...` | ❌ |
| **Cohere** | Command | API Key | ✅ Limited |
| **Together AI** | Llama 2 70B | API Key | ✅ Limited |
| **Groq** | Llama 3 70B, Mixtral 8x7B | `gsk_...` | ✅ Yes |
| **Hugging Face** | Mistral 7B, Various | `hf_...` | ✅ Limited |

## Configuration

The configuration file is automatically created at:
- **Linux/macOS**: `~/.config/ruff/ruff.toml`
- **Windows**: `%APPDATA%\ruff\ruff\config\ruff.toml`

The session data is stored at:
- **Linux/macOS**: `~/.local/share/ruff/sessions`
- **Windows**: `%APPDATA%\ruff\ruff\data\sessions`

### Example Configuration

```toml
default_model = "groq-llama3"
max_tokens = 4096
temperature = 0.7

[api_keys]
openai = "sk-your-openai-api-key-here"
anthropic = "sk-ant-your-anthropic-api-key-here"
cohere = "your-cohere-api-key-here"
together = "your-together-api-key-here"
groq = "gsk_your-groq-api-key-here"
huggingface = "hf_your-huggingface-api-key-here"

[theme]
primary_color = "#CE422B"
secondary_color = "#8B4513"
accent_color = "#FF6347"
error_color = "#DC143C"
success_color = "#228B22"
```

## Usage

### Basic Commands

- **Start Ruff**: `ruff`
- **Initialize config**: `ruff --init`
- **Show config**: `ruff --show-config`
- **List sessions**: `ruff --list-sessions`
- **Delete a session**: `ruff --delete-session <SESSION_ID>`

### In-Chat Controls

| Key Combination | Action |
|----------------|--------|
| `Enter` | Send message |
| `Ctrl+M` | Switch AI model |
| `Ctrl+C` | Quit application |
| `↑/↓` | Navigate model selector |
| `Page Up/Down` | Scroll chat history |
| `Home/End` | Move cursor to start/end |

### Model Selection

Press `Ctrl+M` to open the model selector. Use arrow keys to navigate and `Enter` to select:

```
Select AI Model (↑↓ to navigate, Enter to select, Esc to cancel)
┌─────────────────────────────────────────────────────────────┐
│ openai         GPT-4                     8K tokens          │
│ openai         GPT-3.5 Turbo            4K tokens          │
│ anthropic      Claude 3 Sonnet          4K tokens          │
│ groq           Llama 3 70B               8K tokens          │
│ groq           Mixtral 8x7B              32K tokens         │
│ together       Llama 2 70B Chat         4K tokens          │
└─────────────────────────────────────────────────────────────┘
```

## API Key Setup

### Free/Low-Cost Options

1. **Groq** (Recommended for free usage):
   - Visit: https://console.groq.com/
   - Very fast inference, generous free tier
   - Great for testing and development

2. **Hugging Face**:
   - Visit: https://huggingface.co/settings/tokens
   - Free tier available for many models
   - Good for experimental models

3. **Together AI**:
   - Visit: https://api.together.xyz/
   - Credits-based pricing, free trial
   - Access to many open-source models

### Paid Options

4. **OpenAI**:
   - Visit: https://platform.openai.com/api-keys
   - Industry-leading models
   - Pay-per-use pricing

5. **Anthropic**:
   - Visit: https://console.anthropic.com/
   - Claude models with large context
   - Competitive pricing

6. **Cohere**:
   - Visit: https://dashboard.cohere.ai/api-keys
   - Command models for various tasks
   - Free tier available

## Features in Detail

### Token Usage Tracking

Ruff provides detailed token usage information for each message:

```
📊 Tokens - In: 25 | Out: 150 | Total: 175
Messages: 8 | Total Tokens: In: 420 Out: 1,250 Total: 1,670
```

### Error Handling

Comprehensive enhanced error system with:
- **Rich Context**: Error IDs, timestamps, stack traces, operation info
- **Categories**: Network, Storage, Config, API, UI, Plugin, Message, Session, Performance, Unknown
- **Severity Levels**: Info, Warning, Error, Critical
- **Builder Pattern**: Quick constructors for common error types
- **Automatic Conversions**: From trait implementations for seamless error handling
- Detailed error messages for:
  - Invalid API keys
  - Network issues
  - Rate limiting
  - Token limit exceeded
  - Model availability

### Chat Sessions

- **Session Management**: Multiple concurrent sessions with unique IDs
- **Automatic Timestamping**: Created/updated timestamps for all sessions
- **Message History Preservation**: Persistent storage with message threading
- **Token Usage Tracking**: Per-message and session-level token accounting
- **Model Switching**: Change models mid-conversation without losing context
- **Search & Filter**: Full-text search across sessions with advanced filters
- **Tags & Metadata**: Organize sessions with tags and rich metadata
- **Archive System**: Archive old sessions without deleting them
- **Lazy Loading**: Load sessions on-demand for better performance

### Message Features

- **Message Threading**: Parent-child relationships for conversation branches
- **Message Editing**: Edit previous messages with edit timestamp tracking
- **Message Deletion**: Delete messages with optional cascade to children
- **Message Search**: Search within messages with fuzzy matching
- **Message Versions**: Track message regeneration and version history
- **Clipboard Support**: Copy messages to system clipboard

### Export & Import System

**Export Formats:**
- **Markdown**: Human-readable format with code highlighting
- **JSON**: Machine-readable with complete metadata
- **HTML**: Styled web format for sharing
- **Text**: Plain text for universal compatibility

**Backup & Restore:**
- **Compressed Backups**: TAR + GZIP compression (60-80% size reduction)
- **SHA-256 Checksums**: Cryptographic integrity verification
- **Automatic Verification**: Checksum validation on restore
- **Metadata Preservation**: Complete session state with timestamps
- **Configuration Backup**: Include app config in backups
- **Plugin Backup**: Backup installed plugins

### Plugin System

- **Plugin Manager**: Install, uninstall, enable/disable plugins
- **Security Sandbox**: Isolated execution with permission system
- **UI Extensions**: Plugins can extend the user interface
- **Slash Commands**: Custom commands via plugins
- **Plugin Registry**: Centralized plugin management
- **Error Isolation**: Plugin errors don't crash the app
- **Hot Reload**: Update plugins without restart (planned)

### Advanced UI Features

- **Command Palette**: Quick access to all commands (Ctrl+Shift+P)
- **Help System**: Context-sensitive help (F1)
- **Theme System**: Customizable color schemes
- **Markdown Rendering**: Rich text display for AI responses
- **Syntax Highlighting**: Code blocks with language detection
- **Virtual Scrolling**: Smooth scrolling for long conversations
- **Parameter Adjustment**: Real-time model parameter tuning
- **System Prompt Editor**: Interactive system prompt management
- **Accessibility**: Screen reader support and keyboard navigation
- **Font Size Control**: Zoom in/out for readability

### Performance Features

- **Async Runtime**: Tokio-based async/await for non-blocking I/O
- **Rate Limiting**: Smart rate limiting per provider
- **Background Indexing**: Non-blocking search index updates
- **Lazy Loading**: Load data on-demand to reduce memory usage
- **Streaming Support**: Server-Sent Events (SSE) for real-time responses
- **Connection Pooling**: Reuse HTTP connections for efficiency
- **Benchmarking Tools**: Built-in performance profiling

## Development

### Building from Source

```bash
git clone https://github.com/yourusername/ruff
cd ruff
cargo build --release
```

### Running Tests

```bash
cargo test
```

### Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library exports
├── app.rs               # Main application logic
├── models.rs            # AI model definitions
├── api.rs               # API client implementations
├── chat.rs              # Chat session management
├── ui.rs                # Terminal user interface
├── events.rs            # Event bus system
├── streaming.rs         # SSE streaming support
├── templates.rs         # Template management
├── config_legacy.rs     # Legacy configuration
├── app/
│   └── app_integration_tests.rs
├── config/              # Configuration system
│   ├── mod.rs           # Configuration module
│   ├── models.rs        # Model configurations
│   ├── service.rs       # Configuration service
│   ├── parameters.rs    # Parameter management
│   ├── network.rs       # Network settings
│   ├── rate_limiter.rs  # Rate limiting
│   └── validation.rs    # Config validation
├── error/               # Error handling
│   ├── mod.rs           # Error module
│   └── enhanced_error.rs # Enhanced error types
├── session/             # Session management
│   ├── manager.rs       # Session manager
│   ├── metadata.rs      # Session metadata
│   ├── search.rs        # Session search
│   ├── system_prompt.rs # System prompt management
│   └── lazy_loading.rs  # Lazy session loading
├── message/             # Message handling
│   ├── manager.rs       # Message manager
│   ├── operations.rs    # Message operations
│   ├── search.rs        # Message search
│   └── threading.rs     # Message threading
├── export/              # Export/Import system
│   ├── service.rs       # Export service
│   ├── import.rs        # Import functionality
│   ├── backup.rs        # Backup/restore with compression
│   ├── formats.rs       # Export formats (MD/JSON/HTML)
│   └── validation.rs    # Export validation
├── plugin/              # Plugin system
│   ├── manager.rs       # Plugin manager
│   ├── traits.rs        # Plugin traits
│   ├── security.rs      # Plugin security sandbox
│   ├── registry.rs      # Plugin registry
│   ├── slash_commands.rs # Slash command support
│   ├── ui_extensions.rs # UI extension points
│   └── error_isolation.rs # Error isolation
├── search/              # Search & indexing
│   ├── index.rs         # Search index
│   ├── query.rs         # Query parsing
│   ├── ranking.rs       # Result ranking
│   ├── fuzzy.rs         # Fuzzy search
│   └── background_indexing.rs # Background indexer
├── ui/                  # Enhanced UI components
│   ├── enhanced/
│   │   ├── layout.rs    # Layout manager
│   │   ├── palette.rs   # Command palette
│   │   ├── help.rs      # Help system
│   │   ├── themes.rs    # Theme service
│   │   ├── markdown.rs  # Markdown renderer
│   │   ├── syntax.rs    # Syntax highlighter
│   │   ├── clipboard.rs # Clipboard manager
│   │   ├── navigation.rs # Navigation manager
│   │   ├── accessibility.rs # Accessibility features
│   │   ├── parameters.rs # Parameter adjustment UI
│   │   └── system_prompt.rs # System prompt UI
│   └── virtual_scrolling.rs # Virtual scrolling
├── logging/             # Structured logging
│   ├── mod.rs
│   └── structured_logger.rs
└── performance/         # Performance utilities
    ├── mod.rs
    └── benchmarks.rs    # Performance benchmarks
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Areas for Contribution

- Additional AI model providers
- Enhanced UI features
- Performance optimizations
- Documentation improvements
- Test coverage

## Troubleshooting

### Common Issues

**Config file not found**:
```bash
ruff --init  # Initialize configuration
```

**Invalid API key**:
- Check that your API key is correctly formatted
- Ensure you have sufficient credits/quota
- Verify the key has the required permissions

**Network errors**:
- Check your internet connection
- Verify API endpoints are accessible
- Some providers may have regional restrictions

**Token limit exceeded**:
- Try using a model with higher token limits
- Clear chat history to reduce context size
- Reduce the `max_tokens` setting in config

### Debug Mode

Set the `RUST_LOG` environment variable for detailed logging:

```bash
RUST_LOG=debug ruff
```

## License

This project is licensed under the MIT OR Apache-2.0 license.

## Acknowledgments

- Built with [Tokio](https://tokio.rs/) for async runtime
- [Ratatui](https://ratatui.rs/) for terminal UI
- [Crossterm](https://github.com/crossterm-rs/crossterm) for cross-platform terminal
- [Serde](https://serde.rs/) for serialization
- [Reqwest](https://github.com/seanmonstar/reqwest) for HTTP client

---

Made with ❤️ and 🦀 Rust