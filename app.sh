#!/usr/bin/env bash
# app.sh — Synaptic Signals CMS management script
# Usage: ./app.sh <command>
#
# Commands:
#   start          Build (if needed) and start the server in the background
#   stop           Stop the running server
#   restart        Stop then start (no rebuild)
#   rebuild        Stop, build, then start
#   status         Show whether the server is running
#   logs           Tail live server logs (Ctrl+C to exit)
#   build          Compile a debug build
#   build-release  Compile an optimised release build
#   update-cli     Reinstall synaptic-cli after CLI source changes
#   migrate        Run pending database migrations
#   clean-index    Delete the Tantivy search index (rebuilt on next start)
#   clean-build    Delete the Cargo target/ directory to force a full rebuild
#   test           Run unit tests (no database required)
#   test-all       Run unit tests + integration tests (requires DATABASE_URL)

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="$SCRIPT_DIR/.synaptic.pid"
LOG_FILE="$SCRIPT_DIR/logs/synaptic.log"
BINARY="$SCRIPT_DIR/target/debug/synaptic"
SEARCH_INDEX="$SCRIPT_DIR/search-index"

# Read PORT from .env if present, default to 3000
PORT=3000
if [[ -f "$SCRIPT_DIR/.env" ]]; then
    _port=$(grep -E '^PORT=' "$SCRIPT_DIR/.env" 2>/dev/null | cut -d= -f2 | tr -d '[:space:]' || true)
    [[ -n "$_port" ]] && PORT="$_port"
fi

# Make sure cargo is available
if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
fi

if ! command -v cargo &>/dev/null; then
    echo "ERROR: cargo not found. Install Rust: https://rustup.rs" >&2
    exit 1
fi

# ── Helpers ───────────────────────────────────────────────────────────────────

log() { echo "[$(date '+%H:%M:%S')] $*"; }

is_running() {
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(<"$PID_FILE")
        kill -0 "$pid" 2>/dev/null
    else
        return 1
    fi
}

free_port() {
    if fuser "${PORT}/tcp" &>/dev/null 2>&1; then
        log "Port ${PORT} is in use — clearing..."
        fuser -k "${PORT}/tcp" 2>/dev/null || true
        sleep 1
    fi
}

remove_pid() {
    rm -f "$PID_FILE"
}

check_postgres() {
    local db_url="${DATABASE_URL:-}"
    if [[ -z "$db_url" && -f "$SCRIPT_DIR/.env" ]]; then
        db_url=$(grep -E '^DATABASE_URL=' "$SCRIPT_DIR/.env" 2>/dev/null | cut -d= -f2- | tr -d '[:space:]' || true)
    fi

    if [[ -z "$db_url" ]]; then
        log "WARNING: DATABASE_URL not set — skipping PostgreSQL connectivity check."
        return 0
    fi

    # Parse host and port from postgres://user:pass@host:port/dbname
    local host port
    host=$(echo "$db_url" | sed -E 's|.*@([^:/]+)[:/].*|\1|')
    port=$(echo "$db_url" | sed -E 's|.*@[^:]+:([0-9]+)/.*|\1|')
    [[ "$port" =~ ^[0-9]+$ ]] || port=5432

    if command -v pg_isready &>/dev/null; then
        if ! pg_isready -h "$host" -p "$port" -q 2>/dev/null; then
            log "ERROR: PostgreSQL is not reachable at ${host}:${port}"
            log "Start PostgreSQL before starting the server."
            exit 1
        fi
    else
        # Fallback: TCP check via bash /dev/tcp
        if ! (echo > /dev/tcp/"$host"/"$port") 2>/dev/null; then
            log "ERROR: PostgreSQL is not reachable at ${host}:${port}"
            log "Start PostgreSQL before starting the server."
            exit 1
        fi
    fi
    log "PostgreSQL is reachable at ${host}:${port}"
}

# ── Commands ──────────────────────────────────────────────────────────────────

cmd_start() {
    if is_running; then
        log "Already running (PID $(<"$PID_FILE")). Use 'restart' to restart."
        exit 0
    fi

    check_postgres

    # Ensure binary exists; build if missing
    if [[ ! -f "$BINARY" ]]; then
        log "Binary not found — building..."
        cmd_build
    fi

    mkdir -p "$SCRIPT_DIR/logs"

    # Clear stale port
    free_port

    # Remove any leftover lock files from a previous crash
    if [[ -d "$SEARCH_INDEX" ]]; then
        find "$SEARCH_INDEX" -name "*.lock" -delete 2>/dev/null || true
    fi

    log "Starting Synaptic Signals..."
    cd "$SCRIPT_DIR"
    nohup "$BINARY" >> "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"

    # Wait briefly and confirm it's still alive
    sleep 2
    if is_running; then
        log "Started (PID $(<"$PID_FILE")) — listening on port ${PORT}"
        log "Logs: $LOG_FILE"
    else
        log "ERROR: Server failed to start. Check logs:"
        tail -20 "$LOG_FILE"
        remove_pid
        exit 1
    fi
}

cmd_stop() {
    if ! is_running; then
        log "Not running."
        remove_pid
        return 0
    fi

    local pid
    pid=$(<"$PID_FILE")
    log "Stopping server (PID $pid)..."
    kill "$pid" 2>/dev/null || true

    # Wait up to 5 seconds for graceful shutdown
    local i=0
    while kill -0 "$pid" 2>/dev/null && (( i < 10 )); do
        sleep 0.5
        i=$(( i + 1 ))
    done

    # Force kill if still alive
    if kill -0 "$pid" 2>/dev/null; then
        log "Force killing..."
        kill -9 "$pid" 2>/dev/null || true
    fi

    remove_pid
    free_port
    log "Stopped."
}

cmd_restart() {
    cmd_stop
    sleep 1
    cmd_start
}

cmd_rebuild() {
    cmd_stop
    cmd_build
    sleep 1
    cmd_start
}

cmd_status() {
    if is_running; then
        log "Running (PID $(<"$PID_FILE")) on port ${PORT}"
    else
        log "Not running."
        remove_pid
    fi
}

cmd_logs() {
    if [[ ! -f "$LOG_FILE" ]]; then
        log "No log file found at $LOG_FILE — has the server been started yet?"
        exit 1
    fi
    echo "Tailing $LOG_FILE (Ctrl+C to exit)..."
    tail -f "$LOG_FILE"
}

cmd_build() {
    log "Building (debug)..."
    cd "$SCRIPT_DIR"
    cargo build --bin synaptic
    log "Build complete: $BINARY"
}

cmd_build_release() {
    log "Building (release)..."
    cd "$SCRIPT_DIR"
    cargo build --release --bin synaptic
    BINARY="$SCRIPT_DIR/target/release/synaptic"
    log "Build complete: $BINARY"
}

cmd_update_cli() {
    log "Reinstalling synaptic-cli..."
    cd "$SCRIPT_DIR"
    cargo install --path cli
    log "synaptic-cli updated: $(command -v synaptic-cli)"
}

cmd_dev_reset() {
    if ! command -v synaptic-cli &>/dev/null; then
        log "synaptic-cli not found — run './app.sh update-cli' first."
        exit 1
    fi
    cd "$SCRIPT_DIR"
    synaptic-cli dev reset
}

cmd_migrate() {
    if ! command -v synaptic-cli &>/dev/null; then
        log "synaptic-cli not found — run './app.sh update-cli' first."
        exit 1
    fi
    cd "$SCRIPT_DIR"
    log "Running database migrations..."
    synaptic-cli migrate
}

cmd_clean_index() {
    if is_running; then
        log "Server is running — stop it first before clearing the search index."
        exit 1
    fi
    if [[ -d "$SEARCH_INDEX" ]]; then
        rm -rf "$SEARCH_INDEX"
        log "Search index deleted. It will be rebuilt on next start."
    else
        log "No search index found at $SEARCH_INDEX."
    fi
}

cmd_clean_build() {
    log "Deleting target/ directory (this will cause a full rebuild next time)..."
    cd "$SCRIPT_DIR"
    cargo clean
    log "Done."
}

cmd_test() {
    log "Running unit tests (no database required)..."
    cd "$SCRIPT_DIR"
    cargo test -p synaptic-core -p admin
    log "Done."
}

cmd_test_all() {
    if [[ -z "${DATABASE_URL:-}" ]]; then
        log "ERROR: DATABASE_URL is not set. Integration tests require a live PostgreSQL instance."
        log "Example: DATABASE_URL=postgres://user:pass@localhost/synaptic_signals ./app.sh test-all"
        exit 1
    fi
    log "Running all tests including integration tests (DATABASE_URL is set)..."
    cd "$SCRIPT_DIR"
    SQLX_OFFLINE=false cargo test -p synaptic-core \
        --test model_crud \
        --test theme_e2e \
        -- --include-ignored
    cargo test -p synaptic-core -p admin
    log "Done."
}

# ── Dispatch ──────────────────────────────────────────────────────────────────

COMMAND="${1:-help}"

case "$COMMAND" in
    start)         cmd_start ;;
    stop)          cmd_stop ;;
    restart)       cmd_restart ;;
    rebuild)       cmd_rebuild ;;
    status)        cmd_status ;;
    logs)          cmd_logs ;;
    build)         cmd_build ;;
    build-release) cmd_build_release ;;
    update-cli)    cmd_update_cli ;;
    migrate)       cmd_migrate ;;
    dev-reset)     cmd_dev_reset ;;
    clean-index)   cmd_clean_index ;;
    clean-build)   cmd_clean_build ;;
    test)          cmd_test ;;
    test-all)      cmd_test_all ;;
    help|--help|-h)
        echo ""
        echo "Usage: ./app.sh <command>"
        echo ""
        echo "Server:"
        echo "  start          Build (if needed) and start server in background"
        echo "  stop           Stop the running server"
        echo "  restart        Stop then start (no rebuild)"
        echo "  rebuild        Stop, build, then start (use after code changes)"
        echo "  status         Show whether the server is running"
        echo "  logs           Tail live server logs (Ctrl+C to exit)"
        echo ""
        echo "Build:"
        echo "  build          Compile debug build"
        echo "  build-release  Compile optimised release build"
        echo "  update-cli     Reinstall synaptic-cli after CLI source changes"
        echo ""
        echo "Development:"
        echo "  dev-reset      Wipe all DB data (keeps schema/migrations) for a clean install run"
        echo ""
        echo "Maintenance:"
        echo "  migrate        Run pending database migrations"
        echo "  clean-index    Delete Tantivy search index (rebuilt on next start)"
        echo "  clean-build    Delete target/ to force a full recompile"
        echo ""
        echo "Testing:"
        echo "  test           Run unit tests (no database required)"
        echo "  test-all       Run unit + integration tests (requires DATABASE_URL env var)"
        echo ""
        ;;
    *)
        echo "Unknown command: $COMMAND"
        echo "Run './app.sh help' for usage."
        exit 1
        ;;
esac
