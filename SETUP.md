# Configuration du serveur MCP (Antigravity / Claude)

Ce document explique comment configurer le serveur `mcp-meta-indexer` pour l'utiliser en local avec ton agent IA (Claude, Antigravity, etc.).

## 1. Prérequis

- **Rust / Cargo** installé sur ta machine (`rustup`).
- **Ripgrep** (`rg`) installé et accessible dans ton PATH.

## 2. Compilation

Avant de pouvoir utiliser le serveur, il est recommandé de le tester ou de le compiler en mode release pour des performances optimales :

```bash
cd mcp-meta-indexer
cargo build --release
```

## 3. Configuration dans Antigravity / Claude

Pour que l'agent IA détecte et utilise ce serveur, il faut le déclarer dans le fichier de configuration MCP de ton workspace (ou au niveau global).

Dans le répertoire racine de ton projet (ex: `.agents/mcp_config.json`), ajoute la configuration suivante :

```json
{
  "mcpServers": {
    "meta-indexer-local": {
      "command": "cargo",
      "args": ["run", "--release", "--manifest-path", "mcp-meta-indexer/Cargo.toml"],
      "env": {
        "MCP_TRANSPORT": "stdio"
      }
    }
  }
}
```

## 4. Règle de comportement (Recommandé)

Pour forcer l'agent à utiliser ce serveur plutôt que ses outils de recherche par défaut (`grep_search` / `rg`), ajoute une règle explicite dans ton fichier `.agents/AGENTS.md` (ou similaire) :

> **SERVEUR MCP OBLIGATOIRE (`meta-indexer`)** : Interdiction d'utiliser `grep_search` ou des commandes bash `rg`. Tu DOIS obligatoirement utiliser les outils fournis par le serveur MCP (`smart_search` et `find_dependents`).

## 5. Vérification

Recharge ton éditeur (ou démarre une nouvelle session IA). Tu devrais voir les outils du MCP apparaître dans les capacités natives de l'agent.
