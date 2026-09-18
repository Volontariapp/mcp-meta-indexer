# Priorité 5 : Telemetry-Aware AST (Couplage Datadog/OTEL)

## Objectif
Donner à l'agent IA la capacité de profiler le code. Le MCP enrichit les retours AST avec de vraies métriques de production pour orienter l'IA vers des optimisations réelles.

## Architecture & Fonctionnement
1. Le serveur MCP se connecte à l'API de monitoring de production (ex: Datadog, Prometheus, ou OpenTelemetry).
2. Lorsque l'IA demande l'AST d'un contrôleur ou d'un resolver GraphQL, le MCP identifie le nom de l'endpoint.
3. Le MCP fait une requête à Datadog pour obtenir le temps de réponse p99 (99ème percentile) et le taux d'erreur sur les dernières 24 heures.
4. Ces informations sont injectées dynamiquement sous forme de commentaires JSDoc fictifs au-dessus de la fonction cible avant de l'envoyer à l'IA.

## Changements Requis

### 1. `src/integrations/datadog.rs` (Nouveau)
- Création d'un client HTTP asynchrone (`reqwest`) pour appeler l'API de métriques.
- Gestion de l'authentification (Tokens injectés via Secrets K8s).

### 2. Mapping Endpoint <-> Métriques
- Il faut une logique pour mapper le nom de la classe/fonction (`UserController.getUsers`) vers la métrique correspondante dans le dashboard.

### 3. Injection dans l'AST
- Modifier le filtre de compression (actuellement `ast_parser.rs`) pour concaténer les commentaires de télémétrie : `// [TELEMETRY] Latence moyenne: 400ms`.

## Sécurité
- Nécessite l'exposition de credentials de monitoring sensibles (API Keys) dans l'environnement du Pod.
