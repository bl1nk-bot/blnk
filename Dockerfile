ARG VARIANT="22.04"
FROM mcr.microsoft.com/devcontainers/base:ubuntu-${VARIANT}

ARG DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    ca-certificates \
    curl \
    wget \
    git \
    git-lfs \
    openssh-client \
    openssh-server \
    gnupg \
    lsb-release \
    apt-transport-https \
    software-properties-common \
    jq \
    yq \
    htop \
    tmux \
    vim \
    nano \
    less \
    tree \
    unzip \
    zip \
    tar \
    gzip \
    bzip2 \
    xz-utils \
    libssl-dev \
    libffi-dev \
    libreadline-dev \
    zlib1g-dev \
    libbz2-dev \
    libsqlite3-dev \
    postgresql-client \
    mysql-client \
    sqlite3 \
    redis-tools \
    && rm -rf /var/lib/apt/lists/*

# Add deadsnakes PPA and install Python 3.11 as default
RUN add-apt-repository ppa:deadsnakes/ppa -y \
    && apt-get update && apt-get install -y --no-install-recommends \
    python3.11 \
    python3.11-dev \
    python3.11-venv \
    python3.11-distutils \
    && rm -rf /var/lib/apt/lists/* \
    && update-alternatives --install /usr/bin/python3 python3 /usr/bin/python3.11 1 \
    && update-alternatives --install /usr/bin/python python /usr/bin/python3.11 1 \
    && python3.11 -m ensurepip --upgrade \
    && python3.11 -m pip install --no-cache-dir --upgrade \
    pip \
    setuptools \
    wheel \
    black \
    flake8 \
    pylint \
    pytest \
    pytest-cov \
    pytest-asyncio \
    pytest-mock \
    mypy \
    ruff \
    poetry \
    pipenv \
    virtualenv

# Install Docker CLI
RUN curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null \
    && apt-get update && apt-get install -y --no-install-recommends \
    docker-ce-cli \
    && rm -rf /var/lib/apt/lists/*

# Install Ona CLI
RUN curl -fsSL https://ona.com/install.sh | bash \
    && ona --version

# Install additional development tools
RUN apt-get update && apt-get install -y --no-install-recommends \
    make \
    cmake \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Create workspace directory
RUN mkdir -p /workspace && chown -R vscode:vscode /workspace

# Switch to vscode user for remaining setup
USER vscode

# Install Node.js 22 + 24 (via nvm) and global npm packages including bun
ENV NVM_DIR="/home/vscode/.nvm"
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash \
    && . "$NVM_DIR/nvm.sh" \
    && nvm install 22 \
    && nvm install 24 \
    && nvm alias default 22 \
    && npm install -g \
    npm@latest \
    yarn@latest \
    pnpm@latest \
    bun \
    tsx \
    ts-node \
    nodemon \
    pm2 \
    && echo "node: $(node --version)" \
    && echo "bun: $(bun --version)"

# Install Rust 1.97 via rustup
ENV RUSTUP_HOME="/home/vscode/.rustup"
ENV CARGO_HOME="/home/vscode/.cargo"
ENV PATH="/home/vscode/.cargo/bin:${PATH}"
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.97.0 --profile default -w \
    && rustc --version \
    && cargo --version

# Install Hermes Agent (non-interactive — skip setup wizard and browser/Playwright)
RUN . "$NVM_DIR/nvm.sh" \
    && curl -fsSL https://raw.githubusercontent.com/NousResearch/hermes-agent/main/scripts/install.sh | bash -s -- --skip-setup --skip-browser \
    && hermes --version

# Configure shell
RUN echo 'export NVM_DIR="$HOME/.nvm"' >> ~/.bashrc \
    && echo '[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"' >> ~/.bashrc \
    && echo '[ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"' >> ~/.bashrc \
    && echo '. "$HOME/.cargo/env"' >> ~/.bashrc \
    && echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc

# Verify all installed versions
RUN . "$NVM_DIR/nvm.sh" \
    && echo "=== Installed Versions ===" \
    && echo "Node.js (default): $(node --version)" \
    && echo "Node.js 22: $(nvm exec 22 node --version 2>/dev/null | tail -1)" \
    && echo "Node.js 24: $(nvm exec 24 node --version 2>/dev/null | tail -1)" \
    && echo "npm: $(npm --version)" \
    && echo "bun: $(bun --version)" \
    && echo "pnpm: $(pnpm --version)" \
    && echo "yarn: $(yarn --version)" \
    && echo "Python: $(python3.11 --version)" \
    && echo "Rust: $(rustc --version)" \
    && echo "Cargo: $(cargo --version)" \
    && echo "Hermes Agent: $(hermes --version)" \
    && echo "========================"

WORKDIR /workspace
