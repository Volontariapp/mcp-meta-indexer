# mcp-meta-indexer

Serveur MCP (Model Context Protocol) hybride pour l'indexation et la recherche dans la codebase Volontariapp.
Ce serveur expose des outils pour les environnements de développement locaux et les exécutions distantes (Kubernetes).

## CI/CD

Ce projet est intégré à la boucle de synchronisation globale `ci-tools`. 
À chaque push sur `main`, l'intégration continue Github Actions (`.github/workflows/ci.yml`) s'exécute pour :
1. Compiler et vérifier le code Rust via `cargo build`, `cargo test` et `clippy`.
2. Construire l'image Docker Alpine optimisée.
3. Déployer l'image sur le cluster via la mise à jour du repository `deploy`.
