# Deep Dive: AST Parsing avec Tree-Sitter

Le fichier `ast_parser.rs` contient l'intelligence sémantique du serveur MCP.

## Principes de fonctionnement

1. **Chargement Dynamique du Langage :**
   - Le parser Rust supporte 4 langages via Tree-sitter : `Typescript`, `Rust`, `JSON`, et `YAML`.
   - **Important :** Les versions des crates `tree-sitter-*` doivent être alignées sur `tree-sitter = "0.23"` (compatible `edition2024`).
   - L'instanciation se fait via `LANGUAGE.into()` (trait `LanguageFn`).

2. **Extraction de Contexte (Skeletonization) :**
   - L'objectif n'est pas de renvoyer tout le fichier, mais seulement le **bloc logique englobant** la ligne ciblée par `ripgrep` (ex: la classe entière, ou la fonction entière).
   - L'AST remonte des nœuds enfants vers les nœuds parents (`class_declaration`, `function_item`, etc.) jusqu'à trouver un conteneur logique.

3. **Minification RTK (Rust Token Killer) :**
   - Pour économiser les tokens envoyés au LLM, le code extrait passe par `minify_rtk_style`.
   - Les commentaires simples `//` sont supprimés.
   - Les lignes vides sont condensées.
   - **Exception :** Les commentaires de documentation (JSDoc `/**` et RustDoc `///`) sont préservés car ils contiennent de l'information utile.
