#!/usr/bin/env bash
# Install & Update Script for blnk on Linux
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/bl1nk-bot/blnk/main/scripts/install.sh | bash
#   ./scripts/install.sh --auto-update
#   ./scripts/install.sh --update

set -euo pipefail

REPO="bl1nk-bot/blnk"
TARGET_DIR="${HOME}/.blnk/bin"
CONFIG_DIR="${HOME}/.blnk"
UPDATE_MODE="manual"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --auto-update)
      UPDATE_MODE="auto"
      shift
      ;;
    --manual-update)
      UPDATE_MODE="manual"
      shift
      ;;
    --update)
      shift
      ;;
    *)
      shift
      ;;
  esac
done

echo "==> Checking latest blnk release..."
RELEASE_JSON=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")
TAG=$(echo "${RELEASE_JSON}" | grep '"tag_name":' | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')

ARCH=$(uname -m)
if [ "$ARCH" = "x86_64" ]; then
  TARGET="x86_64-unknown-linux-gnu"
elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
  TARGET="aarch64-unknown-linux-gnu"
else
  echo "Unsupported architecture: $ARCH"
  exit 1
fi

TAR_URL=$(echo "${RELEASE_JSON}" | grep "browser_download_url.*${TARGET}.tar.gz" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$TAR_URL" ]; then
  echo "Could not find asset for target: ${TARGET}"
  exit 1
fi

echo "==> Downloading blnk ${TAG} (${TARGET})..."
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

curl -fsSL "$TAR_URL" -o "${TMP_DIR}/blnk.tar.gz"
tar -xzf "${TMP_DIR}/blnk.tar.gz" -C "${TMP_DIR}"

mkdir -p "${TARGET_DIR}"
find "${TMP_DIR}" -type f -name "blnk" -exec cp -f {} "${TARGET_DIR}/blnk" \;
chmod +x "${TARGET_DIR}/blnk"

echo "==> Installed blnk to ${TARGET_DIR}/blnk"

# Setup PATH
SHELL_CONFIG=""
if [ -n "${BASH_VERSION:-}" ]; then
  SHELL_CONFIG="${HOME}/.bashrc"
elif [ -n "${ZSH_VERSION:-}" ]; then
  SHELL_CONFIG="${HOME}/.zshrc"
else
  SHELL_CONFIG="${HOME}/.profile"
fi

if ! echo "$PATH" | grep -q "${TARGET_DIR}"; then
  if [ -f "$SHELL_CONFIG" ] && ! grep -q 'export PATH=.*\.blnk/bin' "$SHELL_CONFIG"; then
    echo "==> Adding ${TARGET_DIR} to PATH in ${SHELL_CONFIG}..."
    echo "export PATH=\"${TARGET_DIR}:\$PATH\"" >> "$SHELL_CONFIG"
    echo "==> Added to ${SHELL_CONFIG}. Restart terminal or run 'source ${SHELL_CONFIG}'"
  fi
fi

# Save Update Config
mkdir -p "${CONFIG_DIR}"
cat <<EOF > "${CONFIG_DIR}/update_config.json"
{
  "update_mode": "${UPDATE_MODE}",
  "current_version": "${TAG}",
  "repo": "${REPO}"
}
EOF

echo "==> Update mode set to: ${UPDATE_MODE} (default is manual)"
echo "==> Installation complete! Run 'blnk --version' to start."
