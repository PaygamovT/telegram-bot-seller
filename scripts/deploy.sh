#!/usr/bin/env bash

# ==============================================================================
# Automated ARM64 Cross-Compilation & Deployment Script for Samsung Galaxy Flip 3
# ==============================================================================

set -euo pipefail

# --- Color formatting decorators ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# --- Default Deployment Configurations ---
TARGET_HOST="${DEPLOY_HOST:-192.168.1.100}"
TARGET_PORT="${DEPLOY_PORT:-22}"
TARGET_USER="${DEPLOY_USER:-admin}"
TARGET_PATH="${DEPLOY_PATH:-/home/admin/bot-seller}"
BINARY_NAME="telegram-bot-seller"
TARGET_TRIPLE="aarch64-unknown-linux-gnu"

info "Starting deployment process..."
info "Target device: ${TARGET_USER}@${TARGET_HOST}:${TARGET_PORT}"
info "Target directory: ${TARGET_PATH}"

# 1. Determine compilation builder engine
BUILD_CMD="cargo build --target ${TARGET_TRIPLE} --release"
if command -v cross &> /dev/null; then
    info "Found 'cross' installed. Using Docker-based cross-compilation for guaranteed clean builds."
    BUILD_CMD="cross build --target ${TARGET_TRIPLE} --release"
else
    warn "'cross' command not found. Falling back to standard cargo build."
    warn "Ensure the 'aarch64-linux-gnu-gcc' cross-compiler and target toolchain are installed locally."
fi

# 2. Run compilation target
info "Compiling release binary for ARM64 (${TARGET_TRIPLE})..."
info "Executing: ${BUILD_CMD}"

if ! ${BUILD_CMD}; then
    error "Compilation failed! Check toolchain linkers or compiler outputs."
fi

success "Binary compiled successfully!"

# 3. Define compiled binary path
LOCAL_BINARY="target/${TARGET_TRIPLE}/release/${BINARY_NAME}"
if [ ! -f "${LOCAL_BINARY}" ]; then
    error "Compiled binary not found at expected path: ${LOCAL_BINARY}"
fi

# Show compiled binary metrics
BINARY_SIZE=$(du -h "${LOCAL_BINARY}" | cut -f1)
info "Self-contained stripped binary size: ${BINARY_SIZE}"

# 4. Create target directories on remote device
info "Creating remote deployment directories at: ${TARGET_PATH}"
if ! ssh -p "${TARGET_PORT}" "${TARGET_USER}@${TARGET_HOST}" "mkdir -p ${TARGET_PATH}/data ${TARGET_PATH}/src/migrations" &> /dev/null; then
    error "Failed to connect to the remote device over SSH. Check IP, Port, and authorized keys."
fi

# 5. Transfer assets securely (via rsync if available, otherwise scp)
info "Transferring compiled binary and assets..."
if command -v rsync &> /dev/null; then
    info "Using rsync for delta-transfers..."
    rsync -avz -e "ssh -p ${TARGET_PORT}" \
        "${LOCAL_BINARY}" \
        "src/migrations/" \
        ".env.example" \
        "${TARGET_USER}@${TARGET_HOST}:${TARGET_PATH}/"
else
    info "rsync not found. Falling back to scp..."
    scp -P "${TARGET_PORT}" "${LOCAL_BINARY}" "${TARGET_USER}@${TARGET_HOST}:${TARGET_PATH}/"
    scp -P "${TARGET_PORT}" -r src/migrations/ "${TARGET_USER}@${TARGET_HOST}:${TARGET_PATH}/src/"
    scp -P "${TARGET_PORT}" .env.example "${TARGET_USER}@${TARGET_HOST}:${TARGET_PATH}/"
fi

success "Files transferred successfully!"

# 6. Remote systemd service reload & restart
info "Triggering remote service daemon reloads and restarts..."
SSH_RESTART_CMD="sudo systemctl daemon-reload && sudo systemctl restart telegram-bot"

if ssh -p "${TARGET_PORT}" "${TARGET_USER}@${TARGET_HOST}" "${SSH_RESTART_CMD}" &> /dev/null; then
    success "Remote systemd service restarted successfully!"
else
    warn "Failed to trigger automatic service restart over SSH."
    warn "Ensure the systemd service 'telegram-bot.service' is installed and your SSH user has passwordless sudo for systemctl."
    warn "Otherwise, log into the Flip 3 Chroot Debian environment manually and run:"
    warn "  sudo systemctl daemon-reload && sudo systemctl restart telegram-bot"
fi

success "Deployment of '${BINARY_NAME}' completed successfully! 🚀"
