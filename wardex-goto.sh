#!/bin/bash
# Wardex shell integration for quick folder navigation
# Add to your ~/.bashrc or ~/.zshrc:
#   source /path/to/wardex-goto.sh

wx() {
    if [ "$1" = "goto" ]; then
        local path=$(wardex config goto "$2" 2>/dev/null)
        if [ -n "$path" ]; then
            cd "$path" || echo "Failed to navigate to $path"
        else
            echo "Usage: wx goto [workspace|inbox|projects|areas|resources|archives|ctf]"
        fi
    else
        wardex "$@"
    fi
}

# Alias for convenience
alias wxg='wx goto'
