# Priorité 3 : Outils de Refactoring AST-Safe

## Objectif
Fournir à l'agent IA la capacité de modifier la codebase de manière autonome, mais avec des garanties structurelles. Plutôt que de faire un simple "rechercher-remplacer" textuel dangereux, le MCP injecte du code directement dans l'AST.

## Architecture & Fonctionnement
1. Création d'un outil MCP `ast_replace_node`.
2. L'outil prend en entrée : un chemin de fichier, le nom d'une fonction/classe cible, et le nouveau code.
3. Le serveur Rust parse le fichier cible, localise le nœud précis de l'ancienne fonction, et le remplace par le nouveau texte.
4. **Validation critique** : Le MCP re-parse le nouveau fichier entier en mémoire avec Tree-sitter. Si des erreurs de syntaxe (AST Error nodes) sont détectées, l'écriture sur le disque est annulée et un message d'erreur est renvoyé à l'IA pour correction.
5. (Optionnel) Exécution en background d'un linter type `tsc --noEmit` sur le fichier.

## Changements Requis

### 1. `src/tools/ast_refactor.rs` (Nouveau)
- Implémentation du système de mutation de code.
- Utilisation des offsets de bytes (`start_byte`, `end_byte`) fournis par le Tree-sitter de `ast_parser.rs` pour découper et recoller la chaîne de caractères (String manipulation).

### 2. Validation Préventive
- Fonction `validate_syntax(code: &str, lang) -> Result<(), String>`. Si `tree.root_node().has_error()` renvoie `true`, on rejette la modification.

### 3. Tool `ast_replace_node`
- Ajout de l'outil avec le schéma d'input complet dans `main.rs`.

## Dépendances
- Potentiellement l'intégration de `tsc` via l'exécution de commandes système (`std::process::Command`), bien que la validation purement AST soit déjà un énorme bond en avant.
