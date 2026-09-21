#!/bin/bash
# Script appelé par le hook.json (PreToolUse)

# Lecture de la payload envoyée par le moteur Antigravity sur l'entrée standard
PAYLOAD=$(cat)

# Extraction de la commande bash et du Cwd via jq
CMD=$(echo "$PAYLOAD" | jq -r '.toolCall.args.CommandLine')
CWD=$(echo "$PAYLOAD" | jq -r '.toolCall.args.Cwd // empty')

# Si c'est un push ou un commit...
if [[ "$CMD" == *"git push"* || "$CMD" == *"git commit"* ]]; then
    if [ -n "$CWD" ] && [ -d "$CWD" ]; then
        cd "$CWD"
    else
        SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
        cd "$SCRIPT_DIR/../../.."
    fi

    # Run tests & lint en silence s'il s'agit d'un projet Rust
    if [ -f "Cargo.toml" ]; then
        if ! cargo clippy -- -D warnings > /dev/null 2>&1 || ! cargo test > /dev/null 2>&1; then
            echo '{"decision": "deny", "reason": "CI Safety Gate: 🛑 ton code ne compile pas, ne passe pas Clippy, ou les tests échouent. Tu DOIS lancer ./run-ci.sh et corriger les erreurs avant de pouvoir commiter ou pusher !"}'
            exit 0
        fi
    fi
fi

# Sinon, on autorise l'exécution
echo '{"decision": "allow"}'
exit 0
