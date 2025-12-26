# Fork Mechanism Quick Reference

## Command Reference

### Initialize Fork from Network
```bash
# Fork from Testnet (latest version)
aptos move sim init --path sess --network testnet --api-key YOUR_KEY

# Fork from specific version
aptos move sim init --path sess --network testnet \
  --network-version 10000000 --api-key YOUR_KEY

# Fork from Mainnet
aptos move sim init --path sess --network mainnet --api-key YOUR_KEY

# Fork from custom endpoint
aptos move sim init --path sess --network https://custom-node.com --api-key YOUR_KEY
```

### Initialize Local Fork (No Remote)
```bash
# Start with empty genesis state
aptos move sim init --path sess
```

### Fund Account
```bash
aptos move sim fund --session sess --account 0x123... --amount 100000000
```

### View Resource
```bash
aptos move sim view-resource --session sess \
  --account 0x123... --resource 0x1::account::Account
```

### Execute Transaction
```bash
aptos move run --session sess \
  --function-id 0x1::aptos_account::transfer \
  --args address:0x456... u64:50000000
```

## Key Concepts

### 1. Layered Architecture
```
DeltaStateStore (local changes)
    │
    └── Base State View (remote or empty)
```

### 2. State Read Priority
1. **Delta Layer** (local modifications) - Fastest, takes precedence
2. **LRU Cache** (recently fetched) - Fast, avoids network call
3. **Remote Network** (REST API) - Slower, requires network

### 3. State Write Behavior
- All modifications go to **delta layer** (in-memory)
- Delta is **persisted to delta.json** after each operation
- **Remote network is NEVER modified**

### 4. Version Pinning
- Fork is pinned to a specific network version
- All state reads reference the same point in time
- Provides deterministic behavior

## File Structure

```
sess/
├── config.json      # Session configuration (network, version, API key)
├── delta.json       # Local state modifications
└── [N] execute .../ # Transaction execution outputs
    ├── summary.json
    ├── write_set.json
    └── events.json
```

## Code Examples

### Initialize Fork (Programmatic)
```rust
use aptos_transaction_simulation_session::Session;
use url::Url;

let session = Session::init_with_remote_state(
    "sess",                                    // path
    Url::parse("https://fullnode.testnet.aptoslabs.com")?,
    12345678,                                  // version
    Some("your_api_key".to_string()),         // API key
)?;
```

### Load Existing Session
```rust
let mut session = Session::load("sess")?;
```

### Fund Account
```rust
use aptos_types::account_address::AccountAddress;

session.fund_account(
    AccountAddress::from_hex_literal("0x123...")?,
    100_000_000,  // 1 APT in Octa
)?;
```

### Execute Transaction
```rust
use aptos_types::transaction::SignedTransaction;

let (vm_status, output) = session.execute_transaction(txn)?;
```

### View Resource
```rust
use move_core_types::language_storage::StructTag;

let resource = session.view_resource(
    AccountAddress::from_hex_literal("0x123...")?,
    &"0x1::account::Account".parse()?,
)?;
```

## State Access Pattern

```rust
// When reading state:
fn get_state_value(&self, key: &StateKey) -> Option<StateValue> {
    // 1. Check delta first
    if let Some(value) = self.delta.get(key) {
        return value.clone();
    }
    
    // 2. Fall back to base (remote or empty)
    self.base.get_state_value(key)
}
```

## State Modification Pattern

```rust
// When modifying state:
fn set_state_value(&self, key: StateKey, value: StateValue) {
    // Store in delta (never touches remote)
    self.delta.insert(key, Some(value));
    
    // Persist to disk
    save_delta("delta.json", &self.delta)?;
}
```

## Network URLs

| Network | URL |
|---------|-----|
| Mainnet | `https://fullnode.mainnet.aptoslabs.com` |
| Testnet | `https://fullnode.testnet.aptoslabs.com` |
| Devnet  | `https://fullnode.devnet.aptoslabs.com` |

## Common Patterns

### Pattern 1: Test Against Production State
```bash
# Fork from mainnet
aptos move sim init --path sess --network mainnet --api-key KEY

# Test your transaction
aptos move run --session sess --function-id ...
```

### Pattern 2: Debug Specific Version
```bash
# Fork from version where bug occurred
aptos move sim init --path sess --network testnet \
  --network-version 10000000 --api-key KEY

# Reproduce and debug
aptos move run --session sess --function-id ...
```

### Pattern 3: Local Development
```bash
# Start with clean state
aptos move sim init --path sess

# Develop and test locally
aptos move run --session sess --function-id ...
```

## Troubleshooting

| Issue | Solution |
|-------|---------|
| Rate limiting | Add `--api-key` flag |
| Network errors | Check connectivity and API endpoint |
| Version not found | Use more recent version or local fork |
| State not found | Verify account/resource exists at forked version |

## Performance Tips

1. **Use API Key**: Prevents rate limiting
2. **Batch Operations**: Group related reads together
3. **Local for Dev**: Use local fork during development
4. **Remote for Integration**: Use remote fork for integration testing
5. **Recent Versions**: Fork from recent version to minimize divergence

## Important Notes

- ✅ Local modifications are **isolated** from remote network
- ✅ Fork state is **persistent** (can resume sessions)
- ✅ Multiple forks can exist **simultaneously**
- ✅ Delta can be **cleared** to reset fork state
- ❌ Remote network is **never modified**
- ❌ Fork is **pinned** to specific version

## Related Documentation

- [FORK_MECHANISM.md](./FORK_MECHANISM.md) - Detailed explanation
- [FORK_DIAGRAMS.md](./FORK_DIAGRAMS.md) - Visual diagrams
- [examples/fork_example.rs](./examples/fork_example.rs) - Code examples




