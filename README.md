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

Comprehensive error handling for:
- Invalid API keys
- Network issues
- Rate limiting
- Token limit exceeded
- Model availability

### Chat Sessions

- Automatic timestamping
- Message history preservation
- Token usage per message
- Model switching mid-conversation

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
├── main.rs          # CLI entry point
├── lib.rs           # Library exports
├── app.rs           # Main application logic
├── config.rs        # Configuration management
├── models.rs        # AI model definitions
├── api.rs           # API client implementations
├── chat.rs          # Chat session management
├── ui.rs            # Terminal user interface
└── error.rs         # Error types and handling
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