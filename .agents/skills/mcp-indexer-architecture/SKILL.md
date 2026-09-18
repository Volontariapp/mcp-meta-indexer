---
name: mcp-indexer-architecture
description: >-
  Use this skill when you need to understand the architecture, the purpose, and the data flows of the mcp-meta-indexer codebase.
---

# MCP Meta Indexer Architecture

Le **MCP Meta Indexer** est un serveur MCP (Model Context Protocol) écrit en Rust qui agit comme un moteur de recherche hyper-optimisé et un analyseur de code pour l'IA (Claude, Gemini, etc.). Son but est d'indexer et d'explorer les 17 repositories du projet Volontariapp en un temps record (O(1) ou quelques ms).

## Composants Principaux

1. **Smart Search (`smart_search.rs`)** : Moteur hybride combinant la vitesse de `ripgrep` et la précision de `tree-sitter` pour renvoyer des blocs de code pertinents.
   - [Deep dive: Smart Search](./references/smart-search.md)
   
2. **AST Parser (`ast_parser.rs`)** : Utilise `tree-sitter` pour comprendre la sémantique du code (Typescript, Rust, JSON, YAML) et extraire les contextes (fonctions, classes) englobants ainsi que leurs imports. Applique une minification RTK.
   - [Deep dive: AST Parsing](./references/ast-parsing.md)
   
3. **Dependency Graph (`dependency_graph.rs`)** : Maintient en mémoire un graphe inversé (Qui importe quoi ?) permettant des requêtes instantanées en O(1) sur toute la codebase.
   - [Deep dive: Dependency Graph](./references/dependency-graph.md)
