#!/bin/bash
set -e

# Only run in Claude Code web environment
if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
    echo "Not in Claude Code web environment, skipping session setup"
    exit 0
fi

echo "🚀 Setting up DAP GUI development environment for Claude Code web..."

# Install mise (Modern version manager for managing dev tools)
echo "📦 Installing mise..."
if ! command -v mise &> /dev/null; then
    # Install mise from crates.io using cargo
    # This avoids network restrictions on GitHub releases
    cargo install mise --locked

    # mise installs to ~/.cargo/bin which should already be in PATH
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# Verify mise installation
if ! command -v mise &> /dev/null; then
    echo "❌ Error: mise installation failed"
    exit 1
fi

echo "✅ mise installed successfully"

# Trust the mise.toml config file (required for security)
$HOME/.cargo/bin/mise trust

# Install all tools defined in mise.toml
echo "🔧 Installing development tools via mise (python, uv, cargo-nextest, delve, etc.)..."
$HOME/.cargo/bin/mise install

# mise.toml has python.uv_venv_auto = true, so venv should be created automatically
# But let's ensure it exists and has dependencies installed
echo "🐍 Setting up Python environment..."
if [ ! -d ".venv" ]; then
    $HOME/.cargo/bin/mise exec -- uv venv .venv
fi

# Install Python dependencies from requirements.txt using uv
echo "📥 Installing Python dependencies (debugpy, pytest, ipython)..."
$HOME/.cargo/bin/mise exec -- uv pip install -r requirements.txt

# Verify debugpy is installed
if $HOME/.cargo/bin/mise exec -- python -c "import debugpy" 2>/dev/null; then
    echo "✅ debugpy successfully installed and verified"
else
    echo "❌ Error: debugpy installation failed"
    exit 1
fi

# Set up mise activation in bashrc for subsequent commands
echo 'eval "$(~/.cargo/bin/mise activate bash)"' >> "$HOME/.bashrc"

echo "🎉 Session setup complete!"
echo "   - mise installed and configured"
echo "   - Tools installed: python, uv, cargo-nextest, delve, pre-commit, maturin"
echo "   - Virtual environment: .venv"
echo "   - Python packages: debugpy, pytest, ipython installed"
echo "   - mise will be automatically activated in new shells"
