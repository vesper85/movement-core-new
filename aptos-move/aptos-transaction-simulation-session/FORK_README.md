# Fork Mechanism Documentation

This directory contains comprehensive documentation on how the fork mechanism works in Aptos transaction simulation.

## Documentation Files

### 📖 [FORK_MECHANISM.md](./FORK_MECHANISM.md)
**Complete guide** explaining the fork mechanism in detail:
- Architecture overview
- Step-by-step initialization flow
- State access and modification patterns
- Transaction execution flow
- Code examples with explanations
- Advanced usage scenarios
- Troubleshooting guide

**Best for**: Understanding how the fork mechanism works internally

### 📊 [FORK_DIAGRAMS.md](./FORK_DIAGRAMS.md)
**Visual diagrams** showing:
- Architecture overview
- State read/write flows
- Transaction execution flow
- Fork initialization flow
- Delta layer structure
- Session persistence structure
- Comparison: Local vs Remote fork

**Best for**: Visual learners and quick reference

### ⚡ [FORK_QUICK_REFERENCE.md](./FORK_QUICK_REFERENCE.md)
**Quick reference guide** with:
- Command reference
- Key concepts summary
- Code snippets
- Common patterns
- Troubleshooting tips
- Performance tips

**Best for**: Quick lookups and getting started fast

### 💻 [examples/fork_example.rs](./examples/fork_example.rs)
**Runnable code example** demonstrating:
- Fork initialization
- State reading
- Local modifications
- Transaction execution

**Best for**: Learning by example

## Quick Start

### 1. Fork from Testnet
```bash
aptos move sim init --path sess --network testnet --api-key YOUR_KEY
```

### 2. Fund an Account
```bash
aptos move sim fund --session sess --account 0x123... --amount 100000000
```

### 3. Execute Transaction
```bash
aptos move run --session sess --function-id 0x1::aptos_account::transfer \
  --args address:0x456... u64:50000000
```

## What is Forking?

Forking allows you to create a **local simulation environment** based on the state of a remote Aptos network (Mainnet, Testnet, or Devnet). This enables you to:

- ✅ Test transactions against real network state
- ✅ Experiment safely without affecting production
- ✅ Debug issues with deterministic replay
- ✅ Develop features against specific network versions

## How It Works (TL;DR)

1. **Initialize**: Fork from a specific network version
2. **Read State**: System checks local modifications first, then fetches from remote
3. **Modify State**: All changes are stored locally in a delta layer
4. **Execute**: Transactions see the combination of remote state + local modifications
5. **Persist**: Session state is saved to disk for resumability

## Architecture at a Glance

```
DeltaStateStore (local changes)
    │
    └── DebuggerStateView (remote state fetcher)
            │
            └── RestDebuggerInterface (REST API client)
                    │
                    └── Remote Aptos Network
```

## Key Concepts

1. **Layered State**: Delta layer (local) + Base view (remote or empty)
2. **On-Demand Fetching**: State fetched from remote only when needed
3. **Version Pinning**: Fork is pinned to a specific network version
4. **Isolation**: Local modifications never affect remote network
5. **Persistence**: Session state saved to disk for resumability

## Reading Order

1. **New to forking?** Start with [FORK_QUICK_REFERENCE.md](./FORK_QUICK_REFERENCE.md)
2. **Want to understand internals?** Read [FORK_MECHANISM.md](./FORK_MECHANISM.md)
3. **Prefer visuals?** Check [FORK_DIAGRAMS.md](./FORK_DIAGRAMS.md)
4. **Learn by doing?** See [examples/fork_example.rs](./examples/fork_example.rs)

## Related Code

- **Session Implementation**: `src/session.rs`
- **State Store**: `src/state_store.rs`
- **Delta Management**: `src/delta.rs`
- **Configuration**: `src/config.rs`

## Examples

See the `examples/` directory:
- `local.sh` - Local fork example
- `remote.sh` - Remote fork example
- `fork_example.rs` - Programmatic fork example

## Contributing

When updating the fork mechanism:
1. Update relevant documentation files
2. Add examples for new features
3. Update diagrams if architecture changes
4. Keep quick reference up to date

## Questions?

- Check [FORK_MECHANISM.md](./FORK_MECHANISM.md) for detailed explanations
- See [FORK_QUICK_REFERENCE.md](./FORK_QUICK_REFERENCE.md) for common issues
- Review [examples/](./examples/) for usage patterns




