# Configuration du serveur MCP (Antigravity / Claude)

Ce document explique comment configurer et connecter le serveur `mcp-meta-indexer` à ton agent IA (Claude Code, Antigravity, Cursor, etc.).

Tu as le choix entre deux modes de fonctionnement :
- **Mode Local** : Exécute le binaire Rust directement sur ta machine (idéal pour travailler hors-ligne ou modifier le code du MCP).
- **Mode Distant / Cluster** : Exécute les commandes directement dans le Pod déployé sur le cluster Kubernetes (zéro compilation locale, idéal si tu n'as pas Rust installé).

---

## 1. Option A : Configuration pour le Mode Local

### Prérequis
- **Rust & Cargo** installés sur ta machine via [rustup.rs](https://rustup.rs/).
- **Ripgrep** (`rg`) installé et accessible dans ton PATH (`brew install ripgrep` sur Mac).

### Compilation
Compile le binaire en mode release pour des performances optimales :
```bash
cd mcp-meta-indexer
cargo build --release
```

---

## 2. Option B : Configuration pour le Mode Distant (Cluster Kubernetes & kubectl)

Le cluster Kubernetes de Volontariapp n'étant pas exposé sur l'Internet public, l'accès à l'API server Kubernetes et aux Pods s'effectue exclusivement au travers d'un réseau privé maillé sécurisé (**Tailscale**).

### Étape A : Installation & Connexion au VPN Tailscale
1. **Télécharger Tailscale** :
   - Installe l'application Tailscale sur ta machine (Mac, Linux ou Windows) via le site officiel : [tailscale.com/download](https://tailscale.com/download).
2. **Création de compte & Invitation** :
   - Crée-toi un compte Tailscale.
   - Transmets ton adresse email a Victor pour recevoir l'invitation dans le Tailnet privé de Volontariapp.
3. **Connexion dans le terminal** :
   ```bash
   tailscale login
   # ou
   tailscale up
   ```
4. **Vérification de la connectivité réseau** :
   Demande l'IP Tailscale du nœud control-plane, puis vérifie la connexion :
   ```bash
   tailscale ping <IP_TAILSCALE_CONTROL_PLANE>
   ```
   Tu dois recevoir une confirmation `pong from <node-name> (...) in XXms`.

### Étape B : Configuration de `kubectl` & Kubeconfig
1. **Installer `kubectl`** sur ta machine :
   - macOS : `brew install kubectl`
   - Linux : `sudo apt-get install -y kubectl`
   - Windows : `winget install Kubernetes.kubectl`
2. **Installer le fichier `kubeconfig`** :
   - Récupère le fichier de configuration `config` transmis de façon sécurisée.
   - Place ce fichier dans ton dossier personnel `~/.kube/` :
     ```bash
     mkdir -p ~/.kube
     cp /chemin/vers/config_recu ~/.kube/config
     ```
3. **Sécuriser les permissions du fichier (Obligatoire)** :
   ```bash
   chmod 600 ~/.kube/config
   ```
4. **Valider l'accès au cluster** :
   ```bash
   kubectl get nodes
   ```
   Tu dois voir les nœuds du cluster s'afficher avec le statut `Ready` :
   ```text
   NAME               STATUS   ROLES           AGE    VERSION
   <control-plane>    Ready    control-plane   ...    v1.35.x+k3s1
   ```

---

## 3. Configuration dans Antigravity / Claude Code

Pour que l'agent IA se connecte au serveur MCP, tu dois ajouter la déclaration du serveur dans tes paramètres.

### A. Emplacement de la Configuration

- **Dans Antigravity IDE** :
  - **Via l'interface graphique** : Ouvre les **Paramètres (Settings)** de l'IDE $\rightarrow$ onglet **MCP Servers** $\rightarrow$ clique sur *Add Server*.
  - **Via le fichier global Antigravity** : Ajoute la configuration dans `~/.gemini/antigravity-ide/mcp_config.json` (ou `~/.gemini/config/mcp_config.json`).
  - **Via le workspace du projet** : Ajoute directement dans `.agents/mcp_config.json` à la racine de `meta`.
- **Dans Claude Code / Claude Desktop** :
  - Ajoute dans `~/.claude/mcp.json` ou dans ton fichier de configuration workspace.

### B. Bloc de Configuration JSON

Sélectionne la configuration adaptée à ton mode :

```json
{
  "mcpServers": {
    "meta-indexer-local": {
      "command": "./mcp-meta-indexer/target/release/mcp-meta-indexer",
      "args": [],
      "env": {
        "MCP_TRANSPORT": "stdio"
      }
    },
    "meta-indexer-cluster": {
      "command": "kubectl",
      "args": [
        "exec",
        "-i",
        "deployment/mcp-meta-indexer",
        "-n",
        "prod",
        "-c",
        "mcp-meta-indexer",
        "--",
        "env",
        "MCP_TRANSPORT=stdio",
        "CODE_ROOT=/code/deploy.git/submodules",
        "mcp-meta-indexer"
      ]
    }
  }
}
```

> **Note :** Tu n'es pas obligé de conserver les deux entrées. Garde `meta-indexer-local` si tu as compilé le binaire en local avec Rust, ou `meta-indexer-cluster` si tu utilises l'accès VPN Tailscale + `kubectl` (aucun binaire ou code Rust nécessaire sur ta machine locale).

---

## 4. Règle d'Aiguillage de l'Agent (Recommandé)

Pour garantir que l'agent IA utilise systématiquement le serveur MCP plutôt que de lancer des recherches manuelles coûteuses, assure-toi que ton fichier `.agents/AGENTS.md` contient la consigne d'aiguillage :

> **SERVEUR MCP OBLIGATOIRE (`meta-indexer`)** : Interdiction d'utiliser `grep_search` ou des commandes bash `rg`. Tu DOIS obligatoirement utiliser les outils fournis par le serveur MCP (`smart_search`, `find_dependents`, `analyze_impact`, `analyze_grpc` et `search_docs`).

---

## 5. Vérification Finale

Recharge ton éditeur ou démarre une nouvelle session IA. Tu devrais voir les 5 outils du MCP apparaître dans les capacités natives de ton agent :
- `smart_search`
- `find_dependents`
- `analyze_impact`
- `analyze_grpc`
- `search_docs`
