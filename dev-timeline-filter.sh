#!/bin/sh
# Timeline Filter Development Server
# This script sets up and runs the timeline-filter feed generator in development mode

# HTTP Server Configuration
export HTTP_PORT=4050
export EXTERNAL_BASE=localhost:4050

# Database Configuration
export DATABASE_URL=sqlite://development.db

# Timeline Filter Configuration
export TIMELINE_FEEDS=$(pwd)/timeline_feeds.yml
export TIMELINE_CONSUMER_ENABLE=true
export POLL_INTERVAL=30s

# Cleanup Configuration
export CLEANUP_TASK_ENABLE=true
export CLEANUP_TASK_INTERVAL=1h
export CLEANUP_TASK_MAX_AGE=48h

# Logging
export RUST_LOG=timeline_filter=debug,info
export RUST_BACKTRACE=1
export RUST_LIB_BACKTRACE=1

# Setup: Create database (migrations are embedded and run automatically)
echo "==> Preparing database..."
touch development.db
echo "    Database migrations will run automatically on startup"

# Check if timeline_feeds.yml exists
if [ ! -f "timeline_feeds.yml" ]; then
    echo ""
    echo "WARNING: timeline_feeds.yml not found!"
    echo "Please create it from the example:"
    echo "  cp timeline_feeds.example.yml timeline_feeds.yml"
    echo "Then edit it with your configuration."
    echo ""
    exit 1
fi

# Start the server
echo "==> Starting Timeline Filter..."
echo "==> Server will be available at http://localhost:${HTTP_PORT}"
echo "==> Press Ctrl+C to stop"
echo ""

cargo run --bin supercell
