# Priorité 2 : Graphe d'Impact CQRS / Event-Driven

## Objectif
Cartographier les flux d'événements asynchrones entre les microservices pour prévenir les bugs d'architecture distribuée. L'IA saura quel microservice est impacté si elle modifie un événement émis par un autre microservice.

## Architecture & Fonctionnement
L'architecture de Volontariapp utilise le modèle CQRS et des événements (via Redis/Kafka/RabbitMQ).
1. Étendre le `dependency_graph.rs` pour scanner des patterns spécifiques aux événements (ex: `.emit('EventName')` ou les décorateurs NestJS `@EventPattern('EventName')`).
2. Le graphe en mémoire ne mappe plus seulement les imports, mais aussi une table : `EventName -> Emetteur(s) & Consommateur(s)`.
3. Création d'un nouvel outil `analyze_impact`. L'IA lui donne le nom d'un composant ou d'un événement, et l'outil renvoie l'arbre de dépendance cross-microservices.

## Changements Requis

### 1. `src/tools/dependency_graph.rs`
- Mise à jour de la regex ou utilisation de Tree-sitter au démarrage (plus lent mais plus précis) pour parser les décorateurs NestJS (`@MessagePattern`, `@EventPattern`).
- Ajout d'une structure `RwLock<HashMap<String, EventImpact>>` où `EventImpact` contient `publishers: Vec<String>` et `subscribers: Vec<String>`.
- Création de la logique d'extraction `extract_events(content)`.

### 2. Nouveau tool MCP `analyze_impact`
- Enregistrer le tool dans `main.rs`.
- L'outil prend `target_event` en paramètre et formate une réponse claire : "Cet événement est publié par ms-user et consommé par ms-social et ms-storage".

## Dépendances
Aucune nouvelle dépendance si on utilise `regex`. Si on passe par l'AST complet au démarrage, il faudra optimiser les temps de scan.
