# Priorité 4 : Recherche Sémantique Vectorielle (Embeddings Locaux)

## Objectif
Permettre à l'IA de trouver des portions de code par concept (ex: "Logique de paiement Stripe") sans connaître les mots-clés exacts ni utiliser Ripgrep.

## Architecture & Fonctionnement
1. Intégration d'un moteur d'embedding ultra-léger et rapide exécuté nativement en Rust (via `candle-core` de HuggingFace ou `ort`).
2. Au démarrage, ou via un script CLI dédié, la codebase est découpée en blocs logiques (fonctions, classes) grâce à Tree-sitter.
3. Chaque bloc est transformé en un vecteur (embedding) et stocké dans une base vectorielle in-memory (`hnswlib-rs` ou `qdrant`).
4. L'outil MCP `semantic_search` prend une requête en langage naturel, l'encode en vecteur, et retourne les blocs de code les plus proches (Nearest Neighbors).

## Changements Requis

### 1. `Cargo.toml`
- Ajout de dépendances lourdes : `candle-core`, `candle-nn`, `candle-transformers`, ou `hnswlib-rs`.

### 2. `src/embeddings/engine.rs` (Nouveau)
- Implémentation du chargement d'un modèle d'embedding léger (ex: `all-MiniLM-L6-v2`, ~80MB).
- Indexation asynchrone des 17 repositories (peut prendre du temps, nécessitera probablement un cache persistant sur disque pour éviter de le refaire à chaque démarrage K8s).

### 3. Tool `semantic_vector_search`
- Création de l'outil MCP pour interroger la base.

## Considérations Infra (K8s)
- Le modèle d'embedding (fichier `.bin` ou `.safetensors`) devra être packagé dans l'image Docker ou téléchargé via un init-container.
- Augmentation drastique de la consommation CPU/RAM du pod `mcp-meta-indexer`.
