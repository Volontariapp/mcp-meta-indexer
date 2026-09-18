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
    
    style C fill:#f9f,stroke:#333,stroke-width:2px
    style F fill:#bbf,stroke:#333,stroke-width:2px
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
    
    loop Pour chaque fichier TypeScript trouvé
        MCP->>TS: Parse AST(file.ts)
        TS-->>MCP: Arbre syntaxique
        MCP->>MCP: 1. Extraction de TOUS les imports (Contrats, Dépendances)
        MCP->>MCP: 2. Remontée AST depuis la ligne 42 jusqu'au Bloc Parent (Classe/Méthode)
    end
    
    MCP-->>IA: Mini-Graphe (Imports + Code Bloc Parent)
```

## CI/CD

Ce projet est intégré à la boucle de synchronisation globale `ci-tools`. 
À chaque push sur `main`, l'intégration continue Github Actions (`.github/workflows/ci.yml`) s'exécute pour :
1. Compiler et vérifier le code Rust via `cargo build`, `cargo test` et `clippy`.
2. Construire l'image Docker Alpine optimisée (incluant les extensions C).
3. Déployer l'image sur le cluster via la mise à jour du repository `deploy`.
