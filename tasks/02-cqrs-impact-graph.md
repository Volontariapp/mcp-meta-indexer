# Priorité 2 : Graphe d'Impact Asynchrone (CQRS, Outbox & Event-Driven)

## Objectif
Cartographier les flux asynchrones de bout-en-bout (Outbox -> BullMQ / Redis Streams -> Workers / Post-Processors -> Scatter-Gather WS) basés sur la Source Unique de Vérité (`npm-packages/packages/messaging`) et les packages `domain-*`. 

L'IA saura instantanément :
- Quels microservices ou post-processors réagissent à un événement ou un job.
- Quels flux de compensation (Sagas) et retours WebSockets sont déclenchés.
- Quels contrats de payloads (`EventRegistry`, `JobRegistry`) doivent être mis à jour lors d'une modification.

## Architecture Réelle de Volontariapp

Contrairement aux patterns standards (`.emit()` ou `@EventPattern()`), Volontariapp applique un découpage strict entre 5 piliers :

1. **Source Unique de Vérité (SSOT) : `npm-packages/packages/messaging`**
   - **Événements** : `EventMessagingType` (regroupant `EventEventMessagingType`, `UserEventMessagingType`, `SocialEventMessagingType`, `PostEventMessagingType`).
   - **Jobs** : `JobMessagingType` (regroupant `UserJobType`, `SocialJobType`, `EventsJobType`, `PostJobType`).
   - **WebSockets** : `WebsocketMessagingType` et `WebsocketEventRegistry`.
   - **Registres de Payloads** : `EventRegistry` et `JobRegistry`.
   - **Queues BullMQ & Streams** : `EventsQueue`, `UserQueue`, etc., et enum `Streams` (de `@volontariapp/shared`).

2. **Émetteurs (Producers)**
   - **Événements Outbox Métier** : Insérés en transaction ACID dans PostgreSQL via :
     ```typescript
     EventQueueEntity.createEvent<EventEventMessagingType.EVENT_CREATED>({
       type: EventEventMessagingType.EVENT_CREATED,
       emitter: 'ms-event',
       payload,
       targetServices: [Streams.EVENT_CREATED],
     })
     ```
     Localisés dans `npm-packages/packages/domain-*/src/repositories/*`.
   - **Jobs Outbox & Fallbacks** : Insérés via `JobsOutboxEntity.createJob<K>({ type: jobType, target: EventsQueue.EVENTS, ... })` dans `ms-*/src/.../base.command.controller.ts` et `domain-*`.

3. **Consommateurs de Jobs (`workers-runners`)**
   - Classes décorées de `@Processor(EventsQueue.EVENTS)` étendant `BaseWorker<JobMessagingType>`.
   - Handlers unitaires : `class PublishEventHandler implements IJobHandler<typeof JobMessagingType.PUBLISH_EVENT> { readonly jobType = ... }`.
   - Trigger SQL automatique sur `job_audit` insérant l'audit dans `event_outbox`.

4. **Consommateurs d'Événements (`post-processors-runner`)**
   - Classes étendant `BatchPostProcessor<EventEventMessagingType.EVENT_CREATED>` ou `SinglePostProcessor<...>`.
   - Traitements distribués (mise à jour du graphe Neo4j dans `post-processor-social`, géocodage OSM dans `post-processor-event`, nettoyage outbox).

5. **Rassemblement Scatter-Gather & WebSocket (`ws-service`)**
   - `GatherStateService` dans `ws-service` écoute les streams de feedback avec `correlationId`.
   - Agrégation multi-processeurs (ex: attente 2/2) avant push WebSocket direct au client `nativapp` via `WebsocketMessagingType`.

## Changements Requis dans `mcp-meta-indexer`

### 1. `src/tools/impact_graph.rs` (Nouveau Module Dédié)
Créer un indexeur spécialisé en mémoire :
```rust
pub struct AsyncFlowGraph {
    pub events: HashMap<String, EventFlow>,
    pub jobs: HashMap<String, JobFlow>,
}

pub struct EventFlow {
    pub event_type: String,
    pub payload_interface: String,
    pub stream: Option<String>,
    pub producers: Vec<FlowEndpoint>,
    pub consumers: Vec<ConsumerEndpoint>,
    pub compensations: Vec<String>,
    pub websocket_event: Option<String>,
}

pub struct FlowEndpoint {
    pub service: String,
    pub file: String,
    pub line: usize,
    pub pattern: String, // ex: "EventQueueEntity.createEvent"
}

pub struct ConsumerEndpoint {
    pub runner: String,  // ex: "post-processor-social"
    pub handler_class: String,
    pub file: String,
    pub line: usize,
    pub processing_type: String, // "BatchPostProcessor" | "SinglePostProcessor"
}
```

### 2. Algorithme d'Indexation Multi-Passe
- **Passe 1 (SSOT)** : Scanner `npm-packages/packages/messaging` pour extraire tous les types d'événements, jobs et leurs mappings dans `EventRegistry` / `JobRegistry`.
- **Passe 2 (Émetteurs)** : Scanner `npm-packages/packages/domain-*` et `ms-*` pour localiser les appels `EventQueueEntity.createEvent` et `JobsOutboxEntity.createJob`.
- **Passe 3 (Workers)** : Scanner `workers-runners` pour repérer les `@Processor` et `IJobHandler`.
- **Passe 4 (Post-Processors & WS)** : Scanner `post-processors-runner` et `ws-service` pour repérer les `BatchPostProcessor<E>` et les abonnements WebSocket.
- **Réactivité** : Brancher le File Watcher (`notify`) pour re-scanner à chaud lors des modifications.

### 3. Nouveau Tool MCP : `analyze_impact`
- Schéma d'entrée :
  ```json
  {
    "target": { "type": "string", "description": "Nom de l'événement (ex: EVENT_CREATED), du Job (ex: PUBLISH_EVENT) ou du symbole" }
  }
  ```
- Sortie structurée détaillant :
  - Le contrat et le payload TypeScript.
  - Les producteurs (microservice + méthode de repo).
  - Les consommateurs parallèles (post-processors / workers).
  - La saga de compensation (rollback logique en cas d'échec).
  - La propagation WebSocket vers le client.

## Dépendances & Performance
- Utilise les analyseurs existants (`Tree-sitter` et `regex`).
- Temps d'indexation estimé : < 20 ms au démarrage (zéro modèle lourd, structures en mémoire légères).

