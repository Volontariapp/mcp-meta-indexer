# mcp-meta-indexer

Serveur MCP (Model Context Protocol) hybride pour l'indexation et la recherche dans la codebase Volontariapp.
Ce serveur expose des outils pour les environnements de développement locaux et les exécutions distantes (Kubernetes).

## Architecture & Fonctionnement

Le serveur est conçu avec une architecture hybride qui lui permet de fonctionner aussi bien sur la machine locale d'un développeur que déployé sur un cluster Kubernetes.

### Architecture Hybride (Local vs Distant)

```mermaid
graph TD
    A["Agent IA / Claude"] -->|Transport| B{"Environnement"}
    
    B -->|"Local (Stdio)"| C["Processus binaire Rust"]
    C -->|"Accès direct"| D["Monorepo Local"]
    
    B -->|"Distant (SSE)"| E["Cluster K8s / Tailscale"]
    E -->|"Requêtes HTTP/SSE"| F["Pod mcp-meta-indexer"]
    F -->|Accès| G["Volume Code / Git Sync"]
```

### Mécanique de Recherche Sémantique (`smart_search`)

L'outil principal `smart_search` combine la vitesse de **Ripgrep** et l'intelligence de **Tree-sitter** pour extraire uniquement le contexte pertinent d'une codebase géante (17 repositories) sans exploser la taille du contexte de l'IA.

```mermaid
sequenceDiagram
    participant IA as Agent IA
    participant MCP as mcp-meta-indexer
    participant RG as Ripgrep
    participant TS as Tree-sitter (AST)
    
    IA->>MCP: Call tool smart_search(query)
    MCP->>RG: rg "query" --line-number (Filtrage ultra-rapide)
    RG-->>MCP: file.ts:42: match text
    
    loop Pour chaque fichier trouvé (.ts, .rs, .json, .yaml)
        MCP->>TS: Parse AST
        TS-->>MCP: Arbre syntaxique
        MCP->>MCP: 1. Extraction des imports
        MCP->>MCP: 2. Remontée AST vers le Bloc Parent
        MCP->>MCP: 3. Minification RTK (Suppression espaces/commentaires)
    end
    
    MCP-->>IA: Mini-Graphe compressé (Imports + Code)
```

### Graphe de Dépendance en Mémoire (`find_dependents`)

Pour une résolution instantanée des composants, le serveur maintient un graphe asynchrone des imports.

```mermaid
graph LR
    A["Démarrage MCP"] --> B["Thread d'Indexation (Background)"]
    B --> C["Scan complet des 17 repos"]
    C --> D[("HashMap en Mémoire")]
    
    E["File Watcher (notify)"] -->|Fichier modifié| D
    
    IA["Agent IA"] -->|"Qui importe UserAuthRequest ?"| D
    D -->|"O(1) Résolution"| IA
```

### Déploiement Kubernetes (Git-Sync)

Sur le cluster, le serveur accède aux 17 repositories via le pattern "Sidecar Git-Sync", évitant d'inclure le code source dans l'image Docker.

```mermaid
graph TD
    subgraph Pod mcp-meta-indexer
        A["Conteneur : git-sync"]
        B["Conteneur : mcp-meta-indexer (Rust)"]
        C[("Volume Partagé: /code (emptyDir)")]
    end
    
    D["Github (Monorepo meta)"] -->|"Clone SSH toutes les 60s"| A
    A -->|"Ecriture"| C
    B -->|"Lecture / Recherche (ripgrep + AST)"| C
```

## CI/CD

Ce projet est intégré à la boucle de synchronisation globale `ci-tools`. 
À chaque push sur `main`, l'intégration continue Github Actions (`.github/workflows/ci.yml`) s'exécute pour :
1. Compiler et vérifier le code Rust via `cargo build`, `cargo test` et `clippy`.
2. Construire l'image Docker Alpine optimisée (incluant les extensions C).
3. Déployer l'image sur le cluster via la mise à jour du repository `deploy`.
