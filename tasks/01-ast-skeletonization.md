# Priorité 1 : AST Squeletton (Skeletonization)

## Objectif
Donner à l'agent IA une compréhension architecturale complète d'un fichier sans exploser le budget de tokens. Lorsqu'une recherche renvoie une fonction spécifique, le MCP doit également renvoyer les **signatures** des autres fonctions et classes du fichier.

## Architecture & Fonctionnement
1. Lorsqu'un fichier est parsé via `tree-sitter` (ex: dans `ast_parser.rs`), le nœud cible (la fonction trouvée) est extrait intégralement.
2. Un second parcours de l'AST est effectué à la racine du fichier.
3. Le parcours identifie toutes les déclarations (`class_declaration`, `method_definition`, `function_declaration`, etc.).
4. Pour chaque déclaration (qui n'est pas la cible), on extrait uniquement sa **signature** (le nom, les paramètres, le type de retour) sans le corps de la fonction.
5. On renvoie le tout concaténé : Le bloc ciblé + "Squelette du reste du fichier".

## Changements Requis

### 1. `src/tools/ast_parser.rs`
- Création d'une fonction `extract_skeleton(root_node, source_code) -> String`.
- Dans TypeScript, filtrer sur les kinds : `method_definition` (extraire uniquement jusqu'aux paramètres) et `class_declaration` (extraire jusqu'aux accolades).
- Modifier la structure de retour `ParsedContext` pour inclure un champ `skeleton: String`.

### 2. `src/tools/smart_search.rs`
- Mettre à jour l'affichage formaté du Markdown pour inclure une section `--- Squelette du Fichier ---`.

## Dépendances
Aucune nouvelle dépendance requise. Utilisation intensive de l'API Tree-sitter existante.
