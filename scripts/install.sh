#!/usr/bin/env bash
# Install the dwg-mcp binary from the latest GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/aileron-tech/dwg-mcp/main/scripts/install.sh | /bin/bash
#
# Environment:
#   DWG_MCP_INSTALL_DIR  where to put the binary (default: ~/.local/bin)
#   DWG_MCP_TAG          release tag to install (default: the latest release)
set -euo pipefail

REPO="aileron-tech/dwg-mcp"
BIN_NAME="dwg-mcp"

INSTALL_DIR="${DWG_MCP_INSTALL_DIR:-"${HOME}/.local/bin"}"
TAG="${DWG_MCP_TAG:-""}"
TMP_DIR=""

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "error: missing required command: ${cmd}" >&2
    exit 1
  fi
}

cleanup() {
  if [[ -n "${TMP_DIR}" && -d "${TMP_DIR}" ]]; then
    rm -rf "${TMP_DIR}"
  fi
}

download_file() {
  local url="$1" out="$2"
  if ! curl -fsSL -o "${out}" "${url}"; then
    echo "error: download failed: ${url}" >&2
    exit 1
  fi
}

resolve_tag() {
  if [[ -n "${TAG}" ]]; then
    return 0
  fi
  local tag
  tag="$(
    curl -fsSL -o - "https://api.github.com/repos/${REPO}/releases/latest" \
      | sed -nE 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
      | sed -n '1p'
  )" || true
  if [[ -z "${tag}" ]]; then
    echo "error: failed to resolve the latest release tag of ${REPO}" >&2
    exit 1
  fi
  TAG="${tag}"
}

resolve_target() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"
  case "${os}-${arch}" in
    Darwin-arm64 | Darwin-aarch64) echo "aarch64-apple-darwin" ;;
    Darwin-x86_64) echo "x86_64-apple-darwin" ;;
    Linux-x86_64) echo "x86_64-unknown-linux-gnu" ;;
    Linux-aarch64 | Linux-arm64) echo "aarch64-unknown-linux-gnu" ;;
    *)
      echo "error: no prebuilt ${BIN_NAME} for ${os} ${arch}" >&2
      echo "hint: build from source: cargo install --git https://github.com/${REPO}" >&2
      exit 1
      ;;
  esac
}

sha256_of() {
  local file="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file}" | awk '{print $1}'
  else
    shasum -a 256 "${file}" | awk '{print $1}'
  fi
}

main() {
  require_cmd curl
  require_cmd tar
  require_cmd uname
  trap cleanup EXIT

  local target archive base
  target="$(resolve_target)"
  resolve_tag
  archive="${BIN_NAME}-${target}.tar.gz"
  base="https://github.com/${REPO}/releases/download/${TAG}"

  TMP_DIR="$(mktemp -d)"
  echo "Installing ${BIN_NAME} ${TAG} (${target})"
  download_file "${base}/${archive}" "${TMP_DIR}/${archive}"
  download_file "${base}/${archive}.sha256" "${TMP_DIR}/${archive}.sha256"

  local expected actual
  expected="$(awk '{print $1}' "${TMP_DIR}/${archive}.sha256")"
  actual="$(sha256_of "${TMP_DIR}/${archive}")"
  if [[ -z "${expected}" || "${expected}" != "${actual}" ]]; then
    echo "error: checksum mismatch for ${archive}" >&2
    echo "  expected: ${expected}" >&2
    echo "  actual:   ${actual}" >&2
    exit 1
  fi

  tar -xzf "${TMP_DIR}/${archive}" -C "${TMP_DIR}"
  mkdir -p "${INSTALL_DIR}"
  cp "${TMP_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
  chmod 755 "${INSTALL_DIR}/${BIN_NAME}"

  echo "Installed ${INSTALL_DIR}/${BIN_NAME}"
  case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *) echo "note: ${INSTALL_DIR} is not on your PATH; add it to run ${BIN_NAME} by name" ;;
  esac
  echo "Connect it to Claude Code:  claude mcp add dwg -- ${INSTALL_DIR}/${BIN_NAME}"
}

main "$@"
