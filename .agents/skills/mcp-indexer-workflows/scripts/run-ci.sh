#!/bin/bash
# Script utilitaire pour valider le code (Lint + Tests)

echo "🚀 Lancement des vérifications CI locales..."

echo "1️⃣ Exécution de Cargo Clippy..."
if ! cargo clippy -- -D warnings; then
    echo "❌ Erreur: Clippy a détecté des problèmes. Veuillez les corriger."
    exit 1
fi
echo "✅ Clippy validé !"

echo "2️⃣ Exécution de Cargo Test..."
if ! cargo test; then
    echo "❌ Erreur: Les tests ont échoué. Veuillez les corriger."
    exit 1
fi
echo "✅ Tests validés !"

echo "🎉 Tout est au vert, le code est prêt à être commité !"
exit 0
