---
name: mcp-indexer-workflows
description: >-
  Use this skill to learn about the CI rules, the Docker builds, and the pre-commit checks required when developing in the mcp-meta-indexer codebase.
---

# Workflows & Règles CI (MCP Meta Indexer)

En tant qu'Agent IA, tu dois impérativement respecter les workflows et les règles de validation de ce repo avant de pousser du code.

## 1. Validation du Code (Clippy & Tests)

Il est **STRICTEMENT INTERDIT** de commiter ou pousser du code qui ne passe pas les deux validations suivantes :
1. `cargo clippy -- -D warnings` (Aucun avertissement toléré, tout doit être corrigé ou explicité avec un `#[allow(...)]`).
2. `cargo test` (Tous les tests doivent passer).

Tu peux utiliser le script utilitaire `run-ci.sh` pour lancer ces deux vérifications d'un coup :
[run-ci.sh](./scripts/run-ci.sh)

## 2. Docker & Compilateur Rust

Le projet est packagé dans une image Alpine pour être le plus léger possible.
- **Rust Edition 2024 :** Les dépendances `tree-sitter` exigent un compilateur Rust récent (1.80+ minimum, idéalement `1.85` ou `latest`). 
- **Dockerfile :** Le `Dockerfile` utilise `FROM rust:alpine AS builder` pour supporter les lockfiles `v4` et être compatible `edition2024`.
- [Deep dive: CI & Docker](./references/ci-docker.md)

## 3. Pre-Commit Safety Gate (Hook)

Un Hook d'agent (`hooks.json`) est installé dans ce workspace.
Si tu essaies d'exécuter `git commit` ou `git push` via le tool `run_command`, le script [pre-commit-check.sh](./scripts/pre-commit-check.sh) sera automatiquement déclenché en arrière-plan. 

S'il échoue, l'exécution de ton outil sera **bloquée (deny)** et tu devras réparer le code !
