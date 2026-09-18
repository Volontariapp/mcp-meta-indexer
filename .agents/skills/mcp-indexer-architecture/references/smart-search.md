# Deep Dive: Smart Search

Le fichier `smart_search.rs` implémente le tool MCP `smart_search`.

## Pourquoi un moteur Hybride ?

L'IA classique (ex: un LLM qui utilise `cat` ou `grep` dans un terminal) gaspille d'énormes quantités de tokens en lisant des fichiers entiers. 
`smart_search` résout ce problème en 3 étapes :

1. **Filtrage Ultra-Rapide (`ripgrep`) :**
   - On lance un process `rg` en sous-marin pour trouver exactement dans quels fichiers et à quelles lignes (avec `--line-number`) se trouve la requête.
   - On limite à 20 résultats (pour la performance).

2. **Enrichissement Sémantique :**
   - Pour chaque ligne trouvée, on passe le relais à `ast_parser.rs`.
   - Si le fichier est du TS ou du Rust, on extrait tout le bloc de la fonction concernée + la liste de ses imports en haut du fichier.

3. **Résultat Condensé :**
   - Le résultat final renvoyé à l'IA ressemble à :
     ```text
     === Fichier: src/user.ts ===
     --- Imports ---
     import { Auth } from 'auth';
     --- Contexte Sémantique ---
     class User { constructor() {} }
     ```
   - Si le fichier dépasse 20 000 caractères, il est automatiquement tronqué pour protéger la fenêtre de contexte du LLM.
