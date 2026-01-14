FROM rust:slim-bookworm

# Install dependencies
RUN apt-get update && apt-get install -y git pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Setup workspace
WORKDIR /app
COPY . .

# Build
RUN cargo build --release

# Setup test environment
RUN mkdir -p /test_workspace/0_Inbox /test_workspace/1_Projects /test_workspace/1_Projects/MyRepo
COPY test_env/config.yaml /test_workspace/config.yaml

# Init git repo
WORKDIR /test_workspace/1_Projects/MyRepo
RUN git config --global user.email "test@example.com"
RUN git config --global user.name "Test User"
RUN git init && touch README.md && git add README.md && git commit -m "Initial"

# Test Script
WORKDIR /test_workspace
COPY tests/run_tests.sh .
RUN chmod +x run_tests.sh

CMD ["./run_tests.sh"]
