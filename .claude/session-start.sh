#!/bin/bash
set -e

# Only run in Claude Code web environment
if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
    echo "Not in Claude Code web environment, skipping session setup"
    exit 0
fi

echo "🚀 Setting up DAP GUI development environment for Claude Code web..."

# Create Python virtual environment using uv
echo "🐍 Creating Python virtual environment with uv..."
if [ ! -d ".venv" ]; then
    uv venv .venv
fi

# Install Python dependencies from requirements.txt
echo "📥 Installing Python dependencies (debugpy, pytest, ipython)..."
uv pip install -r requirements.txt

# Activate the virtual environment
echo "✅ Activating virtual environment..."
source .venv/bin/activate

# Verify debugpy is installed
if python -c "import debugpy" 2>/dev/null; then
    echo "✅ debugpy successfully installed and verified"
else
    echo "❌ Error: debugpy installation failed"
    exit 1
fi

# Set environment variable to auto-activate venv in subsequent commands
# This ensures Claude's Bash commands run with the venv active
echo "export VIRTUAL_ENV=\"$(pwd)/.venv\"" >> "$HOME/.bashrc"
echo "export PATH=\"$(pwd)/.venv/bin:\$PATH\"" >> "$HOME/.bashrc"

echo "🎉 Session setup complete!"
echo "   - Virtual environment: .venv"
echo "   - Python packages: debugpy, pytest, ipython installed"
echo "   - Virtual environment will be automatically activated"
