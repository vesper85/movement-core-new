# Aptos Transaction Simulation Fork Mechanism

## Overview

The Aptos transaction simulation system provides a powerful **forking mechanism** that allows you to create a local simulation environment based on the state of a remote Aptos network (Mainnet, Testnet, or Devnet). This enables you to:

- Test transactions against real network state without spending real tokens
- Experiment with smart contracts in a safe, isolated environment
- Debug issues by replaying transactions from production networks
- Develop and test against specific network versions

## Architecture

The fork mechanism uses a **layered state architecture** with two key components:

```
┌─────────────────────────────────────────────────────────────┐
│                    DeltaStateStore                           │
│  (Local modifications layer - stored in delta.json)         │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │           EitherStateView                            │  │
│  │                                                       │  │
│  │  ┌─────────────────────────────────────────────────┐ │  │
│  │  │      DebuggerStateView                         │ │  │
│  │  │  (On-demand remote state fetcher)              │ │  │
│  │  │                                                 │ │  │
│  │  │  Uses RestDebuggerInterface to fetch state     │ │  │
│  │  │  from remote network via REST API              │ │  │
│  │  └─────────────────────────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Key Components

1. **DeltaStateStore**: Tracks local modifications on top of the base state
2. **DebuggerStateView**: Fetches state from remote network on-demand
3. **RestDebuggerInterface**: REST API client for querying remote state

## How Forking Works

### Step 1: Initialization

When you run:
```bash
aptos move sim init --path sess --network testnet --api-key YOUR_API_KEY
```

The system performs the following steps:

#### 1.1 Network Version Resolution

```rust
// From: crates/aptos/src/move_tool/sim.rs

let network_version = match self.network_version {
    Some(txn_id) => txn_id,  // Use specified version
    None => {
        // Fetch latest version from network
        let client = Client::builder(network.to_base_url()?).build();
        client.get_ledger_information().await?.inner().version
    },
};
```

**Code Example:**
```rust
// If --network-version is not specified, the system:
// 1. Connects to testnet REST API
// 2. Queries /v1/ endpoint for ledger information
// 3. Extracts the latest version number

// Example: Latest testnet version might be 12345678
let network_version = 12345678;
```

#### 1.2 Session Directory Setup

```rust
// From: aptos-transaction-simulation-session/src/session.rs

pub fn init_with_remote_state(
    session_path: impl AsRef<Path>,
    node_url: Url,
    network_version: u64,
    api_key: Option<String>,
) -> Result<Self> {
    let session_path = session_path.as_ref().to_path_buf();
    
    // Create directory if it doesn't exist
    std::fs::create_dir_all(&session_path)?;
    
    // Ensure directory is empty
    if session_path.read_dir()?.next().is_some() {
        anyhow::bail!("Cannot initialize new session -- directory is not empty.");
    }
    // ...
}
```

**Result:** Creates `sess/` directory structure:
```
sess/
├── config.json    # Session configuration
└── delta.json     # Local state modifications (initially empty)
```

#### 1.3 Configuration Creation

```rust
// From: aptos-transaction-simulation-session/src/session.rs

let config = Config::with_remote(
    node_url.clone(),      // e.g., "https://fullnode.testnet.aptoslabs.com"
    network_version,       // e.g., 12345678
    api_key.clone()        // Your API key
);
config.save_to_file(&config_path)?;
```

**config.json Example:**
```json
{
  "base": {
    "Remote": {
      "node_url": "https://fullnode.testnet.aptoslabs.com",
      "network_version": 12345678,
      "api_key": "YOUR_API_KEY"
    }
  },
  "ops": 0
}
```

#### 1.4 REST Client Setup

```rust
// From: aptos-transaction-simulation-session/src/session.rs

let mut builder = Client::builder(AptosBaseUrl::Custom(node_url));
if let Some(api_key) = api_key {
    builder = builder.api_key(&api_key)?;  // Add API key to avoid rate limiting
}
let client = builder.build();
```

**Why API Key?**
- Prevents rate limiting on public endpoints
- Allows higher request throughput
- Required for production use cases

#### 1.5 State Store Creation

```rust
// From: aptos-transaction-simulation-session/src/session.rs

let state_store = DeltaStateStore::new_with_base(
    EitherStateView::Right(
        DebuggerStateView::new(
            Arc::new(RestDebuggerInterface::new(client)),
            network_version,
        )
    )
);
```

**What This Does:**
- Creates a `DeltaStateStore` with an empty delta (no local changes yet)
- Sets the base state view to `DebuggerStateView` which fetches from remote
- The `DebuggerStateView` is pinned to a specific `network_version`

### Step 2: State Access Pattern

When the simulation needs to read state, it follows this pattern:

```rust
// From: aptos-transaction-simulation/src/state_store.rs

impl<V> TStateView for DeltaStateStore<V>
where
    V: TStateView<Key = StateKey>,
{
    fn get_state_value(&self, state_key: &Self::Key) -> StateViewResult<Option<StateValue>> {
        // 1. First, check if we have a local modification
        if let Some(res) = self.states.read().get(state_key) {
            return Ok(res.clone());  // Return local modification
        }
        
        // 2. If no local modification, fetch from base (remote network)
        self.base.get_state_value(state_key)
    }
}
```

**Flow Diagram:**
```
Read Request for StateKey
         │
         ▼
    ┌─────────┐
    │  Delta  │  Check local modifications first
    │  Layer  │
    └─────────┘
         │
         ├─ Found? ──► Return local value
         │
         └─ Not Found
                │
                ▼
         ┌──────────────┐
         │ Remote Fetch │  Fetch from network
         │   (Base)     │
         └──────────────┘
                │
                ▼
         Return remote value
```

**Code Example: Reading Account Balance**

```rust
// When you query an account balance:

// 1. First check delta (local modifications)
let delta = state_store.states.read();
if let Some(Some(balance_value)) = delta.get(&account_state_key) {
    return balance_value;  // Use locally modified value
}

// 2. If not in delta, fetch from remote
let remote_value = debugger_state_view
    .get_state_value(&account_state_key)
    .await?;

// 3. The DebuggerStateView uses RestDebuggerInterface:
let response = rest_client
    .get_account_resource_by_version(
        account_address,
        resource_type,
        network_version - 1  // State at version N is stored at N-1
    )
    .await?;
```

### Step 3: State Modification Pattern

When you modify state (e.g., fund an account, execute a transaction), changes are stored in the delta:

```rust
// From: aptos-transaction-simulation/src/state_store.rs

impl<V> SimulationStateStore for DeltaStateStore<V> {
    fn set_state_value(&self, state_key: StateKey, state_val: StateValue) -> Result<()> {
        // Store modification in delta layer
        self.states.write().insert(state_key, Some(state_val));
        Ok(())
    }
    
    fn apply_write_set(&self, write_set: &WriteSet) -> Result<()> {
        let mut states = self.states.write();
        
        for (state_key, write_op) in write_set.write_op_iter() {
            match write_op.as_state_value() {
                None => {
                    // Deletion: mark as None in delta
                    states.insert(state_key.clone(), None);
                }
                Some(state_val) => {
                    // Modification: store new value in delta
                    states.insert(state_key.clone(), Some(state_val));
                }
            }
        }
        Ok(())
    }
}
```

**Example: Funding an Account**

```rust
// From: aptos-transaction-simulation-session/src/session.rs

pub fn fund_account(&mut self, account: AccountAddress, amount: u64) -> Result<()> {
    // This modifies the fungible store balance
    let (before, after) = self.state_store.fund_apt_fungible_store(account, amount)?;
    
    // The modification is stored in delta.json
    save_delta(&self.path.join("delta.json"), &self.state_store.delta())?;
    
    Ok(())
}
```

**What Happens:**
1. Reads current balance from remote (if not in delta)
2. Adds `amount` to the balance
3. Stores the new balance in the delta layer
4. Saves delta to `delta.json`

**delta.json After Funding:**
```json
{
  "resource_group/0x123.../0x1::object::ObjectGroup": {
    "0x1::fungible_store::FungibleStore": "0x..."
  }
}
```

### Step 4: Transaction Execution

When executing a transaction, the system:

1. **Reads state** from delta (if modified) or remote (if not)
2. **Executes** the transaction using AptosVM
3. **Applies write set** to delta layer
4. **Saves** delta to disk

```rust
// From: aptos-transaction-simulation-session/src/session.rs

pub fn execute_transaction(
    &mut self,
    txn: SignedTransaction,
) -> Result<(VMStatus, TransactionOutput)> {
    // 1. Create VM environment with forked state
    let env = AptosEnvironment::new(&self.state_store);
    let vm = AptosVM::new(&env, &self.state_store);
    
    // 2. Execute transaction
    let (vm_status, vm_output) = vm.execute_user_transaction(
        &resolver,
        &code_storage,
        &txn,
        &log_context,
        &auxiliary_info,
    );
    
    // 3. Apply transaction output to delta
    let txn_output = vm_output.try_materialize_into_transaction_output(&resolver)?;
    self.state_store.apply_write_set(txn_output.write_set())?;
    
    // 4. Save delta and transaction output
    save_delta(&self.path.join("delta.json"), &self.state_store.delta())?;
    save_write_set(&self.state_store, &write_set_path, txn_output.write_set())?;
    
    Ok((vm_status, txn_output))
}
```

## Complete Example: Forking Testnet

Let's walk through a complete example:

### 1. Initialize Fork

```bash
aptos move sim init --path sess --network testnet --api-key YOUR_KEY
```

**What happens:**
```rust
// Pseudo-code flow:

1. Parse command: network=testnet, path=sess, api_key=YOUR_KEY
2. Resolve network URL: "https://fullnode.testnet.aptoslabs.com"
3. Fetch latest version: GET /v1/ → { version: 12345678 }
4. Create directory: sess/
5. Save config.json:
   {
     "base": {
       "Remote": {
         "node_url": "https://fullnode.testnet.aptoslabs.com",
         "network_version": 12345678,
         "api_key": "YOUR_KEY"
       }
     },
     "ops": 0
   }
6. Initialize empty delta.json: {}
7. Create state store with DebuggerStateView pointing to version 12345678
```

### 2. Fund an Account

```bash
aptos move sim fund --session sess --account 0x123... --amount 100000000
```

**What happens:**
```rust
// Pseudo-code:

1. Load session from sess/
2. Read current balance:
   - Check delta.json: not found
   - Fetch from remote: GET /v1/accounts/0x123.../resource/0x1::fungible_store::FungibleStore?version=12345677
   - Returns: { balance: 0 }
3. Modify balance: 0 + 100000000 = 100000000
4. Store in delta:
   delta["resource_group/0x123.../0x1::object::ObjectGroup"] = {
     "0x1::fungible_store::FungibleStore": <new_balance_bytes>
   }
5. Save delta.json
6. Update config.json: { "ops": 1 }
```

### 3. Execute Transaction

```bash
aptos move run --session sess --function-id 0x1::aptos_account::transfer \
  --args address:0x456... u64:50000000
```

**What happens:**
```rust
// Pseudo-code:

1. Load session
2. Build transaction
3. Execute transaction:
   a. Read sender balance from delta (found: 100000000)
   b. Read receiver balance:
      - Check delta: not found
      - Fetch from remote: GET /v1/accounts/0x456.../resource/...?version=12345677
      - Returns: { balance: 5000000 }
   c. Execute transfer logic
   d. Generate write set:
      - Sender: 100000000 - 50000000 = 50000000
      - Receiver: 5000000 + 50000000 = 55000000
4. Apply write set to delta
5. Save delta.json with both account modifications
6. Save transaction output to sess/[1] execute 0x1::aptos_account::transfer/
7. Update config.json: { "ops": 2 }
```

### 4. View Resource

```bash
aptos move sim view-resource --session sess \
  --account 0x123... --resource 0x1::account::Account
```

**What happens:**
```rust
// Pseudo-code:

1. Load session
2. Query resource:
   - Check delta: not found (Account resource not modified)
   - Fetch from remote: GET /v1/accounts/0x123.../resource/0x1::account::Account?version=12345677
   - Returns: Account resource data
3. Deserialize and return as JSON
4. Save summary to sess/[2] view resource .../summary.json
```

## Key Concepts

### 1. Version Pinning

The fork is **pinned to a specific network version**. This means:
- All state reads reference the same point in time
- The remote network can continue evolving, but your fork stays at version 12345678
- This provides deterministic simulation behavior

```rust
// The network_version is fixed at initialization
let state_store = DeltaStateStore::new_with_base(
    DebuggerStateView::new(client, network_version)  // Fixed version
);

// All remote queries use: version - 1
// (State at version N is stored at index N-1)
```

### 2. On-Demand Fetching

State is **not pre-loaded**. Instead:
- State is fetched from remote network only when needed
- Uses LRU cache (1MB) to avoid redundant fetches
- Each fetch is a REST API call to the remote node

```rust
// From: aptos-validator-interface/src/lib.rs

async fn handler_thread(
    db: Arc<dyn AptosValidatorInterface + Send>,
    mut thread_receiver: UnboundedReceiver<...>,
) {
    let cache = Arc::new(Mutex::new(LruCache::new(1024 * 1024)));
    
    loop {
        let (key, version, sender) = thread_receiver.recv().await?;
        
        // Check cache first
        if let Some(val) = cache.lock().unwrap().get(&(key.clone(), version)) {
            sender.send(Ok(val.clone())).unwrap();
        } else {
            // Fetch from remote
            let res = db.get_state_value_by_version(&key, version - 1).await;
            cache.lock().unwrap().put((key, version), res.clone());
            sender.send(res).unwrap();
        }
    }
}
```

### 3. Delta Isolation

Local modifications are **completely isolated**:
- Changes never affect the remote network
- Delta can be discarded to reset to original fork state
- Multiple forks can exist simultaneously

```rust
// Reset fork to original state:
// Simply delete or clear delta.json

// Or programmatically:
let mut session = Session::load("sess")?;
session.state_store.states.write().clear();  // Clear all local changes
save_delta(&session.path.join("delta.json"), &HashMap::new())?;
```

### 4. Persistence

The session state is **persisted to disk**:
- `config.json`: Session metadata and remote network info
- `delta.json`: All local state modifications
- `[N] execute .../`: Transaction execution outputs
- `[N] view .../`: View function results

This allows:
- Resuming sessions after restart
- Inspecting transaction history
- Sharing simulation state with others

## Advanced Usage

### Forking Specific Version

```bash
# Fork from a specific transaction version
aptos move sim init --path sess \
  --network testnet \
  --network-version 10000000 \
  --api-key YOUR_KEY
```

**Use Cases:**
- Reproduce bugs at specific network state
- Test against historical state
- Debug specific transaction sequences

### Local Fork (No Remote)

```bash
# Start with empty genesis state
aptos move sim init --path sess
```

**What happens:**
```rust
// Uses EmptyStateView + Genesis change set
let state_store = DeltaStateStore::new_with_base(
    EitherStateView::Left(EmptyStateView)
);
state_store.apply_write_set(GENESIS_CHANGE_SET_HEAD.write_set())?;
```

### Combining Forks

You can load a session and continue from where you left off:

```rust
// Load existing session
let mut session = Session::load("sess")?;

// Continue simulating
session.fund_account(account, amount)?;
session.execute_transaction(txn)?;
```

## Performance Considerations

### Caching

- **LRU Cache**: 1MB cache for remote state fetches
- **Delta Layer**: All local modifications are in-memory (fast)
- **Persistence**: Delta saved to disk after each operation

### Network Latency

- First access to each state key requires network round-trip
- Subsequent accesses use cache
- API key helps avoid rate limiting delays

### Optimization Tips

1. **Batch Operations**: Group related state reads together
2. **Use API Key**: Prevents rate limiting
3. **Local Testing**: Use local fork for development, remote fork for integration testing
4. **Version Selection**: Fork from recent version to minimize state divergence

## Troubleshooting

### Common Issues

1. **Rate Limiting**
   - **Solution**: Provide API key via `--api-key`
   - **Alternative**: Use local fork for development

2. **Network Errors**
   - **Check**: Network connectivity and API endpoint availability
   - **Verify**: API key is valid and has proper permissions

3. **Version Mismatch**
   - **Issue**: Forked version no longer available on network
   - **Solution**: Fork from a more recent version or use local fork

4. **State Not Found**
   - **Check**: Account/resource exists at forked version
   - **Verify**: Using correct account address and resource type

## Summary

The fork mechanism provides a powerful way to:
- ✅ Test against real network state
- ✅ Experiment safely without affecting production
- ✅ Debug issues with deterministic replay
- ✅ Develop features against specific network versions

The layered architecture (Delta + Remote) ensures:
- Local modifications are isolated
- Remote state is fetched on-demand
- Session state is persistent and resumable
- Performance is optimized with caching

This makes the Aptos transaction simulation system ideal for development, testing, and debugging workflows.

