#!/bin/sh

set -eu

RELEASE="${PRODEX_RELEASE:-latest}"
REPOSITORY="${PRODEX_GITHUB_REPOSITORY:-christiandoxa/prodex}"
NON_INTERACTIVE="${PRODEX_NON_INTERACTIVE:-false}"
MIGRATE="${PRODEX_MIGRATE:-false}"
RUNNING_EXE="${PRODEX_RUNNING_EXE:-}"
BASE_URL_OVERRIDE="${PRODEX_RELEASE_BASE_URL:-}"
NO_PATH_UPDATE="${PRODEX_NO_PATH_UPDATE:-false}"

if [ -n "${PRODEX_INSTALL_DIR:-}" ]; then
  BIN_DIR="$PRODEX_INSTALL_DIR"
  DEFAULT_BIN_DIR=false
else
  : "${HOME:?HOME is required}"
  BIN_DIR="$HOME/.local/bin"
  DEFAULT_BIN_DIR=true
fi

BIN_PATH="$BIN_DIR/prodex"
tmp_dir=""
staged_path=""

step() {
  printf '==> %s\n' "$1"
}

warn() {
  printf 'WARNING: %s\n' "$1" >&2
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --release)
        [ "$#" -ge 2 ] || {
          echo "--release requires a value." >&2
          exit 1
        }
        RELEASE="$2"
        shift
        ;;
      --help | -h)
        cat <<'EOF'
Usage: install.sh [--release VERSION]

Environment:
  PRODEX_RELEASE           Version to install; overridden by --release.
  PRODEX_INSTALL_DIR       Binary directory; defaults to $HOME/.local/bin.
  PRODEX_NON_INTERACTIVE   Set to 1, true, or yes to skip prompts.
EOF
        exit 0
        ;;
      *)
        echo "Unknown argument: $1" >&2
        exit 1
        ;;
    esac
    shift
  done
}

validate_release() {
  if [ "$RELEASE" = "latest" ]; then
    return
  fi
  if ! printf '%s\n' "$RELEASE" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$'; then
    echo "Invalid Prodex release: $RELEASE. Expected latest or x.y.z." >&2
    exit 1
  fi
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "$1 is required to install Prodex." >&2
    exit 1
  }
}

download_file() {
  url="$1"
  output="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$output"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$output" "$url"
  else
    echo "curl or wget is required to install Prodex." >&2
    exit 1
  fi
}

file_sha256() {
  path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print tolower($1)}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print tolower($1)}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$path" | sed 's/^.*= //' | tr 'A-F' 'a-f'
  else
    echo "sha256sum, shasum, or openssl is required to verify Prodex." >&2
    exit 1
  fi
}

prompt_yes_no() {
  prompt="$1"
  case "$NON_INTERACTIVE" in
    1 | [Tt][Rr][Uu][Ee] | [Yy][Ee][Ss]) return 1 ;;
  esac
  if ( : </dev/tty ) 2>/dev/null; then
    printf '%s [y/N] ' "$prompt" >/dev/tty
    IFS= read -r answer </dev/tty || return 1
  else
    return 1
  fi
  case "$answer" in
    y | Y | yes | YES) return 0 ;;
    *) return 1 ;;
  esac
}

existing_prodex_path() {
  if [ -n "$RUNNING_EXE" ]; then
    printf '%s\n' "$RUNNING_EXE"
  else
    command -v prodex 2>/dev/null || true
  fi
}

detect_install_manager() {
  existing_path="$1"
  case "${npm_package_name:-}:$existing_path" in
    @christiandoxa/prodex:* | *:*/node_modules/@christiandoxa/prodex* | *:*/node_modules/@christiandoxa/prodex-*)
      printf 'npm\n'
      return
      ;;
  esac
  if [ -f "$existing_path" ] && head -n 1 "$existing_path" 2>/dev/null | grep -F 'node' >/dev/null 2>&1; then
    printf 'npm\n'
    return
  fi
  case "$existing_path" in
    */.cargo/bin/prodex | */.cargo/bin/prodex.exe)
      if command -v cargo >/dev/null 2>&1 && cargo install --list 2>/dev/null | grep -Eq '^prodex v'; then
        printf 'cargo\n'
        return
      fi
      ;;
  esac
  printf 'direct\n'
}

should_migrate_manager() {
  manager="$1"
  [ "$manager" = "npm" ] || [ "$manager" = "cargo" ] || return 1
  case "$MIGRATE" in
    1 | [Tt][Rr][Uu][Ee] | [Yy][Ee][Ss]) return 0 ;;
  esac
  prompt_yes_no "Replace the existing $manager-managed Prodex with the standalone binary?"
}

migrate_manager() {
  manager="$1"
  case "$manager" in
    npm)
      require_command npm
      if [ "${PRODEX_CODEX_BIN:-codex}" = "codex" ]; then
        step "Preserving Codex as a standalone npm command"
        npm install -g @openai/codex@latest
      fi
      step "Removing npm-managed Prodex"
      npm uninstall -g @christiandoxa/prodex
      ;;
    cargo)
      require_command cargo
      step "Removing cargo-managed Prodex"
      cargo uninstall prodex
      ;;
  esac
}

pick_profile() {
  case "$(uname -s):${SHELL:-}" in
    Darwin:*/zsh) printf '%s\n' "$HOME/.zprofile" ;;
    Darwin:*/bash) printf '%s\n' "$HOME/.bash_profile" ;;
    Linux:*/zsh) printf '%s\n' "$HOME/.zshrc" ;;
    Linux:*/bash) printf '%s\n' "$HOME/.bashrc" ;;
    *) printf '%s\n' "$HOME/.profile" ;;
  esac
}

maybe_add_to_path() {
  case ":$PATH:" in
    *":$BIN_DIR:"*) return ;;
  esac
  case "$NO_PATH_UPDATE" in
    1 | [Tt][Rr][Uu][Ee] | [Yy][Ee][Ss]) return ;;
  esac
  if [ "$DEFAULT_BIN_DIR" != "true" ]; then
    warn "$BIN_DIR is not on PATH; add it before running prodex."
    return
  fi
  profile="$(pick_profile)"
  path_line='export PATH="$HOME/.local/bin:$PATH"'
  if [ -f "$profile" ] && grep -F "$path_line" "$profile" >/dev/null 2>&1; then
    return
  fi
  {
    printf '\n# >>> Prodex installer >>>\n'
    printf '%s\n' "$path_line"
    printf '# <<< Prodex installer <<<\n'
  } >>"$profile"
  step "Added $BIN_DIR to PATH in $profile"
}

cleanup() {
  [ -z "$staged_path" ] || rm -f "$staged_path"
  [ -z "$tmp_dir" ] || rm -rf "$tmp_dir"
}

parse_args "$@"
validate_release
require_command mktemp

case "$(uname -s)" in
  Darwin) os="darwin" ;;
  Linux) os="linux" ;;
  *)
    echo "install.sh supports macOS and Linux." >&2
    exit 1
    ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) arch="x86_64" ;;
  arm64 | aarch64) arch="aarch64" ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

if [ "$os" = "darwin" ]; then
  target="$arch-apple-darwin"
else
  target="$arch-unknown-linux-gnu"
fi
asset="prodex-$target"

if [ -n "$BASE_URL_OVERRIDE" ]; then
  base_url="${BASE_URL_OVERRIDE%/}"
elif [ "$RELEASE" = "latest" ]; then
  base_url="https://github.com/$REPOSITORY/releases/latest/download"
else
  base_url="https://github.com/$REPOSITORY/releases/download/$RELEASE"
fi

tmp_dir="$(mktemp -d)"
trap cleanup 0
trap 'exit 1' 1 2 15
checksums_path="$tmp_dir/SHA256SUMS"
download_path="$tmp_dir/$asset"

step "Downloading Prodex $RELEASE for $target"
download_file "$base_url/SHA256SUMS" "$checksums_path"
expected_digest="$(awk -v asset="$asset" '$2 == asset && length($1) == 64 { print tolower($1); exit }' "$checksums_path")"
if ! printf '%s\n' "$expected_digest" | grep -Eq '^[0-9a-f]{64}$'; then
  echo "Could not find a valid checksum for $asset." >&2
  exit 1
fi
download_file "$base_url/$asset" "$download_path"
actual_digest="$(file_sha256 "$download_path")"
if [ "$actual_digest" != "$expected_digest" ]; then
  echo "Downloaded Prodex checksum did not match." >&2
  echo "expected: $expected_digest" >&2
  echo "actual:   $actual_digest" >&2
  exit 1
fi

mkdir -p "$BIN_DIR"
staged_path="$BIN_DIR/.prodex.$$.new"
cp "$download_path" "$staged_path"
chmod 0755 "$staged_path"
installed_version="$("$staged_path" --version 2>/dev/null || true)"
case "$installed_version" in
  'prodex '*) ;;
  *)
    echo "Downloaded asset did not run as Prodex." >&2
    exit 1
    ;;
esac
if [ "$RELEASE" != "latest" ] && [ "$installed_version" != "prodex $RELEASE" ]; then
  echo "Downloaded asset version $installed_version did not match requested release $RELEASE." >&2
  exit 1
fi

existing_path="$(existing_prodex_path)"
manager="$(detect_install_manager "$existing_path")"
if should_migrate_manager "$manager"; then
  migrate_manager "$manager"
elif [ "$manager" = "npm" ] || [ "$manager" = "cargo" ]; then
  warn "Existing $manager-managed Prodex was left installed; PATH order may select it instead."
fi

mv -f "$staged_path" "$BIN_PATH"
staged_path=""
maybe_add_to_path

step "$installed_version installed at $BIN_PATH"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) step "Current shell: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
