# Deep Dive: CI et Docker (mcp-meta-indexer)

## Le Piège du Lockfile V4
Les versions de `cargo` inférieures à 1.78 ne supportent pas le format `Cargo.lock` V4. Comme le runner Github Actions (ou toi-même en local) utilise une version très récente de Rust pour générer le `Cargo.lock`, il est impératif que le `Dockerfile` utilise lui aussi une version récente (ex: `rust:alpine` ou `rust:1.80-alpine`). 
Sans quoi, la CI Docker crashera avec l'erreur `lock file version 4 was found, but this version of Cargo does not understand this lock file`.

## Optimisations du Dockerfile
Le fichier `Dockerfile` a été extrêmement optimisé pour :
1. **La vitesse (Cache) :** Il compile d'abord un `main.rs` factice avec juste les dépendances. L'astuce est de rajouter `RUN touch src/main.rs` avant de compiler le vrai code, sinon Cargo (voyant un timestamp Git plus vieux que son cache) ignore la compilation !
2. **Le déterminisme :** Utilisation systématique de `cargo build --release --locked`.
3. **Le poids :** Le binaire final Rust fait plusieurs centaines de Mo à cause des symboles de débuggage. Le `Dockerfile` exécute `strip target/release/mcp-meta-indexer` (nécessite `binutils`), ce qui allège l'image Alpine finale de plus de 30 Mo.
