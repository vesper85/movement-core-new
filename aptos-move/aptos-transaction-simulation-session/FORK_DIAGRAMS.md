# Fork Mechanism Visual Diagrams

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Session Directory                            │
│                         (e.g., sess/)                               │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐ │
│  │ config.json  │  │  delta.json  │  │ [N] execute .../          │ │
│  │              │  │              │  │ [N] view .../             │ │
│  │ - base state │  │ - local      │  │ - write_set.json         │ │
│  │ - network    │  │   changes    │  │ - events.json            │ │
│  │ - version    │  │ - state      │  │ - summary.json           │ │
│  │ - api_key    │  │   diffs      │  │                          │ │
│  └──────────────┘  └──────────────┘  └──────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ loads
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Session State Store                            │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              DeltaStateStore                                 │  │
│  │  (Thread-safe HashMap<StateKey, Option<StateValue>>)        │  │
│  │                                                               │  │
│  │  ┌──────────────────────────────────────────────────────┐   │  │
│  │  │         Base State View                               │   │  │
│  │  │                                                         │   │  │
│  │  │  ┌─────────────────────────────────────────────────┐   │   │  │
│  │  │  │    DebuggerStateView                          │   │   │  │
│  │  │  │    - Pinned to network_version                 │   │   │  │
│  │  │  │    - On-demand fetching                       │   │   │  │
│  │  │  │    - LRU cache (1MB)                          │   │   │  │
│  │  │  └─────────────────────────────────────────────────┘   │   │  │
│  │  │              │                                         │   │  │
│  │  │              │ uses                                    │   │  │
│  │  │              ▼                                         │   │  │
│  │  │  ┌─────────────────────────────────────────────────┐   │   │  │
│  │  │  │    RestDebuggerInterface                       │   │   │  │
│  │  │  │    - REST API client                           │   │   │  │
│  │  │  │    - API key support                           │   │   │  │
│  │  │  │    - Async state fetching                      │   │   │  │
│  │  │  └─────────────────────────────────────────────────┘   │   │  │
│  │  └──────────────────────────────────────────────────────┘   │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ fetches from
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Remote Aptos Network                            │
│              (Mainnet/Testnet/Devnet)                               │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │              REST API Endpoint                               │  │
│  │  https://fullnode.{network}.aptoslabs.com                   │  │
│  │                                                               │  │
│  │  GET /v1/accounts/{address}/resource/{type}?version={v}     │  │
│  │  GET /v1/accounts/{address}/resources?version={v}           │  │
│  │  GET /v1/ledger/information                                  │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

## State Read Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    State Read Request                            │
│              get_state_value(StateKey)                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │ DeltaStateStore │
                    └─────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │ Check Delta Layer (in-memory) │
              │   states.read().get(key)       │
              └───────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │                   │
              Found? │                   │ Not Found
                    │                   │
                    ▼                   ▼
        ┌──────────────────┐  ┌──────────────────────┐
        │ Return Local     │  │ Check Base State View │
        │ Value from Delta │  │   base.get_state_value│
        └──────────────────┘  └──────────────────────┘
                                        │
                                        ▼
                            ┌───────────────────────┐
                            │ DebuggerStateView     │
                            │   - Check LRU cache   │
                            └───────────────────────┘
                                        │
                            ┌───────────┴───────────┐
                            │                       │
                      Cache Hit?              Cache Miss?
                            │                       │
                            ▼                       ▼
                ┌──────────────────┐  ┌──────────────────────┐
                │ Return Cached    │  │ RestDebuggerInterface │
                │ Value            │  │   - Build REST request │
                └──────────────────┘  │   - Add API key        │
                                      │   - Send HTTP GET      │
                                      └──────────────────────┘
                                                │
                                                ▼
                                    ┌───────────────────────┐
                                    │ Remote Network        │
                                    │   - Process request   │
                                    │   - Return state      │
                                    └───────────────────────┘
                                                │
                                                ▼
                                    ┌───────────────────────┐
                                    │ Update Cache         │
                                    │ Return Value         │
                                    └───────────────────────┘
```

## State Write Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    State Write Request                           │
│         set_state_value(StateKey, StateValue)                    │
│              OR apply_write_set(WriteSet)                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │ DeltaStateStore │
                    └─────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │ Store in Delta Layer          │
              │   states.write().insert(...)   │
              │   - In-memory HashMap         │
              │   - Thread-safe (RwLock)      │
              └───────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │ Persist to delta.json         │
              │   save_delta(delta_path, ...)  │
              │   - Serialize to JSON          │
              │   - Write to disk              │
              └───────────────────────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │ delta.json      │
                    │ Updated on disk  │
                    └─────────────────┘

Note: Remote network is NEVER modified
      All changes are isolated to delta layer
```

## Transaction Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Transaction Execution                        │
│              execute_transaction(SignedTransaction)             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Create VM Environment                   │
        │   - AptosEnvironment::new(state_store)  │
        │   - AptosVM::new(env, state_store)      │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Execute Transaction                     │
        │   vm.execute_user_transaction(...)      │
        │                                         │
        │   During execution:                     │
        │   - Reads state (delta → remote)        │
        │   - Executes Move bytecode              │
        │   - Generates write set                 │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Apply Write Set to Delta                │
        │   state_store.apply_write_set(...)      │
        │   - All changes go to delta layer        │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Save Transaction Output                 │
        │   - write_set.json                      │
        │   - events.json                         │
        │   - summary.json                        │
        │   - delta.json (updated)                │
        └─────────────────────────────────────────┘
```

## Fork Initialization Flow

```
Command: aptos move sim init --path sess --network testnet --api-key KEY
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Parse Command Arguments                 │
        │   - path: sess                          │
        │   - network: testnet                    │
        │   - api_key: KEY                        │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Resolve Network URL                     │
        │   testnet →                             │
        │   https://fullnode.testnet.aptoslabs.com│
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Fetch Latest Version (if not specified) │
        │   GET /v1/ledger/information            │
        │   → { version: 12345678 }               │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Create Session Directory                │
        │   - Create sess/                        │
        │   - Ensure empty                        │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Create Configuration                    │
        │   Config::with_remote(...)               │
        │   Save to sess/config.json               │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Initialize Empty Delta                  │
        │   Save to sess/delta.json                │
        │   (empty HashMap)                       │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Build REST Client                       │
        │   Client::builder(url)                  │
        │   .api_key(api_key)                     │
        │   .build()                              │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Create State Store                      │
        │   DeltaStateStore::new_with_base(        │
        │     DebuggerStateView::new(              │
        │       RestDebuggerInterface(client),      │
        │       network_version                    │
        │     )                                    │
        │   )                                      │
        └─────────────────────────────────────────┘
                              │
                              ▼
        ┌─────────────────────────────────────────┐
        │ Return Session                          │
        │   Session {                             │
        │     config,                             │
        │     path: sess/,                        │
        │     state_store                         │
        │   }                                     │
        └─────────────────────────────────────────┘
```

## Delta Layer Structure

```
DeltaStateStore
│
├── base: EitherStateView
│   │
│   └── Right: DebuggerStateView
│       │
│       ├── query_sender: Channel for async queries
│       ├── version: 12345678 (pinned)
│       │
│       └── handler_thread: Async worker
│           │
│           ├── LRU Cache (1MB)
│           │   └── (StateKey, Version) → StateValue
│           │
│           └── RestDebuggerInterface
│               └── Client (with API key)
│
└── states: RwLock<HashMap<StateKey, Option<StateValue>>>
    │
    ├── Key: StateKey::resource(...)
    │   └── Value: Some(StateValue)  // Modified
    │
    ├── Key: StateKey::resource(...)
    │   └── Value: None               // Deleted
    │
    └── Key: StateKey::module_id(...)
        └── Value: Some(StateValue)   // New module
```

## Session Persistence Structure

```
sess/
│
├── config.json
│   └── {
│         "base": {
│           "Remote": {
│             "node_url": "https://...",
│             "network_version": 12345678,
│             "api_key": "KEY"
│           }
│         },
│         "ops": 2
│       }
│
├── delta.json
│   └── {
│         "resource/0x1/0x1::account::Account": "0x...",
│         "resource/0x123/0x1::coin::CoinStore": "0x...",
│         ...
│       }
│
├── [0] fund (fungible)/
│   └── summary.json
│       └── {
│             "fund_fungible": {
│               "account": "0x123...",
│               "amount": 100000000,
│               "before": 0,
│               "after": 100000000
│             }
│           }
│
└── [1] execute 0x1::aptos_account::transfer/
    ├── summary.json
    │   └── {
    │         "execute_transaction": {
    │           "status": "Executed",
    │           "gas_used": 100,
    │           "fee_statement": {...}
    │         }
    │       }
    │
    ├── write_set.json
    │   └── [list of state changes]
    │
    └── events.json
        └── [list of emitted events]
```

## Comparison: Local vs Remote Fork

```
┌─────────────────────────────────────────────────────────────────┐
│                    Local Fork (Empty Base)                       │
│                                                                   │
│  DeltaStateStore                                                 │
│    │                                                             │
│    ├── base: EmptyStateView                                      │
│    │   └── Always returns None                                   │
│    │                                                             │
│    └── states: HashMap (local changes)                          │
│        └── Applied on top of genesis state                       │
│                                                                   │
│  Use Case:                                                       │
│    - Integration tests                                           │
│    - Clean slate development                                    │
│    - No network dependency                                       │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                    Remote Fork (Network Base)                    │
│                                                                   │
│  DeltaStateStore                                                 │
│    │                                                             │
│    ├── base: DebuggerStateView                                   │
│    │   └── Fetches from remote network on-demand                 │
│    │                                                             │
│    └── states: HashMap (local changes)                           │
│        └── Applied on top of remote state                        │
│                                                                   │
│  Use Case:                                                       │
│    - Testing against real network state                         │
│    - Debugging production issues                                 │
│    - Experimenting safely                                       │
└─────────────────────────────────────────────────────────────────┘
```

## State Key Resolution Priority

```
State Read Request
        │
        ▼
┌──────────────────┐
│ Priority 1:      │  Check Delta Layer
│ Delta Layer       │  (Local modifications)
│ (In-memory)       │  ✓ Fastest
└──────────────────┘  ✓ Takes precedence
        │
        │ Not found?
        ▼
┌──────────────────┐
│ Priority 2:      │  Check LRU Cache
│ LRU Cache        │  (Recently fetched)
│ (1MB, in-memory)  │  ✓ Fast
└──────────────────┘  ✓ Avoids network call
        │
        │ Not found?
        ▼
┌──────────────────┐
│ Priority 3:      │  Fetch from Remote
│ Remote Network    │  (REST API call)
│ (Network roundtrip)│  ⚠ Slower
└──────────────────┘  ⚠ Requires network
        │
        ▼
    Update Cache
    Return Value
```




