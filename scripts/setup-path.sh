#!/usr/bin/env bash
set -euo pipefail

# Run on the target WSL machine, without sudo.
login_shell=${SHELL:-}
shell_name=${1:-${login_shell##*/}}
case "$shell_name" in
    bash) rc_file="$HOME/.bashrc" ;;
    zsh) rc_file="${ZDOTDIR:-$HOME}/.zshrc" ;;
    *) printf 'Usage: bash scripts/setup-path.sh bash|zsh\n' >&2; exit 1 ;;
esac
if (( $# > 1 )); then
    printf 'Usage: bash scripts/setup-path.sh bash|zsh\n' >&2
    exit 1
fi

path_line='export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'
mkdir -p -- "$HOME/.local/bin"
if [[ -e "$rc_file" && ! -f "$rc_file" ]]; then
    printf 'Not a regular shell startup file: %s\n' "$rc_file" >&2
    exit 1
fi
if [[ ! -f "$rc_file" ]] || ! grep -Fxq -- "$path_line" "$rc_file"; then
    # The leading newline also handles an existing file without a final newline.
    printf '\n# devctl: installed binaries take priority over Cargo binaries.\n%s\n' "$path_line" >> "$rc_file"
fi
printf 'PATH configured in %s. Open a new terminal, or run:\n%s\n' "$rc_file" "$path_line"
