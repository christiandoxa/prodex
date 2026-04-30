#!/bin/bash
# caveman - statusline badge script for Claude Code
# Reads the caveman mode flag file and outputs a colored badge.

CLAUDE_DIR="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
FLAG="$CLAUDE_DIR/.caveman-active"
[ ! -f "$FLAG" ] && exit 0

MODE=$(cat "$FLAG" 2>/dev/null)
if [ "$MODE" = "full" ] || [ -z "$MODE" ]; then
  printf '\033[38;5;172m[CAVEMAN]\033[0m'
else
  SUFFIX=$(echo "$MODE" | tr '[:lower:]' '[:upper:]')
  printf '\033[38;5;172m[CAVEMAN:%s]\033[0m' "$SUFFIX"
fi
