# Deep Dive: Dependency Graph En Mémoire

Le fichier `dependency_graph.rs` implémente le tool MCP `find_dependents`.

## Concept

Au lieu de faire un `ripgrep` à chaque fois que l'IA demande "Qui utilise le contrat `UserAuthRequest` ?", le serveur MCP maintient un graphe inversé en mémoire vive (RAM).

- **Clé (Key) :** Nom du symbole (`UserAuthRequest`) ou du package NPM (`@volontariapp/domain-user`).
- **Valeur (Value) :** Un `HashSet` contenant les chemins relatifs des fichiers qui importent cette clé.

## Extraction via Regex Rapide

Pour des raisons de performance sur 17 repos, l'extraction des dépendances ne passe pas par l'AST de `tree-sitter` (trop lourd à parser sur 10 000 fichiers au démarrage), mais par une expression régulière hautement optimisée (`Regex`).

Elle capture :
- Les exports nommés : `import { A, B } from 'pkg'`
- Les exports par défaut : `import A from 'pkg'`
- Les alias : `import { A as B }` (garde `A`)

## Hot-Reload (Watch)

Le graphe est dynamique. Un `notify::Watcher` tourne en arrière-plan et écoute les événements (Modify/Create) du système de fichiers. Dès qu'un fichier TypeScript est sauvegardé, le graphe est mis à jour instantanément.
