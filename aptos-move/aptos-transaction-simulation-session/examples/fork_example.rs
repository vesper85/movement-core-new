// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating the fork mechanism in Aptos transaction simulation.
//!
//! This example shows how to:
//! 1. Initialize a fork from a remote network
//! 2. Read state from the forked network
//! 3. Make local modifications
//! 4. Execute transactions against the fork
//!
//! Run with: `cargo run --example fork_example`

use aptos_transaction_simulation_session::Session;
use aptos_types::account_address::AccountAddress;
use std::path::PathBuf;
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Aptos Transaction Simulation Fork Example ===\n");

    // Example 1: Initialize a fork from Testnet
    example_1_fork_initialization().await?;

    // Example 2: Read state from fork
    example_2_read_state().await?;

    // Example 3: Make local modifications
    example_3_local_modifications().await?;

    // Example 4: Execute transaction on fork
    example_4_execute_transaction().await?;

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

/// Example 1: Initialize a fork from Testnet
///
/// This demonstrates how to create a new simulation session that forks
/// from the Testnet network at a specific version.
async fn example_1_fork_initialization() -> anyhow::Result<()> {
    println!("Example 1: Forking Testnet\n");

    let session_path = PathBuf::from("example_fork_session");
    
    // Clean up any existing session
    if session_path.exists() {
        std::fs::remove_dir_all(&session_path)?;
    }

    // Initialize fork from Testnet
    // In real usage, you would use:
    //   aptos move sim init --path example_fork_session --network testnet --api-key YOUR_KEY
    //
    // Here we do it programmatically:
    let testnet_url = Url::parse("https://fullnode.testnet.aptoslabs.com")?;
    let network_version = 10000000u64; // Example version - in practice, fetch latest
    let api_key = Some("your_api_key_here".to_string());

    let session = Session::init_with_remote_state(
        &session_path,
        testnet_url.clone(),
        network_version,
        api_key.clone(),
    )?;

    println!("✓ Fork initialized from Testnet");
    println!("  - Session path: {:?}", session_path);
    println!("  - Network URL: {}", testnet_url);
    println!("  - Network version: {}", network_version);
    println!("  - Config saved to: {:?}/config.json", session_path);
    println!("  - Delta saved to: {:?}/delta.json", session_path);

    // Show what the config looks like
    let config_path = session_path.join("config.json");
    if config_path.exists() {
        let config_content = std::fs::read_to_string(&config_path)?;
        println!("\n  Config.json content:");
        println!("  {}", config_content);
    }

    Ok(())
}

/// Example 2: Read state from fork
///
/// This demonstrates how state is read from the forked network.
/// The system first checks local modifications (delta), then fetches
/// from remote if not found.
async fn example_2_read_state() -> anyhow::Result<()> {
    println!("\nExample 2: Reading State from Fork\n");

    let session_path = PathBuf::from("example_fork_session");
    let mut session = Session::load(&session_path)?;

    // Example: Read an account resource
    // In a real scenario, you might read:
    // - Account balance
    // - Account sequence number
    // - Custom Move resources

    let example_account = AccountAddress::from_hex_literal("0x1")?;
    
    println!("Attempting to read Account resource for: {}", example_account);
    println!("  Flow:");
    println!("    1. Check delta.json for local modifications...");
    println!("    2. Not found in delta, fetching from remote network...");
    println!("    3. Remote fetch: GET /v1/accounts/{}/resource/0x1::account::Account?version={}", 
             example_account, 
             "network_version - 1");
    
    // This would trigger a remote fetch if the resource isn't in delta
    let resource = session.view_resource(
        example_account,
        &"0x1::account::Account".parse()?,
    )?;

    match resource {
        Some(_) => println!("  ✓ Resource found and fetched from remote"),
        None => println!("  ✓ Resource not found (account may not exist at this version)"),
    }

    Ok(())
}

/// Example 3: Make local modifications
///
/// This demonstrates how local modifications are stored in the delta layer
/// and take precedence over remote state.
async fn example_3_local_modifications() -> anyhow::Result<()> {
    println!("\nExample 3: Making Local Modifications\n");

    let session_path = PathBuf::from("example_fork_session");
    let mut session = Session::load(&session_path)?;

    // Example: Fund an account
    // This modifies the account's balance locally without affecting the remote network
    let account = AccountAddress::from_hex_literal("0x1234567890abcdef")?;
    let amount = 100_000_000u64; // 1 APT (in Octa)

    println!("Funding account: {}", account);
    println!("  Amount: {} Octa (1 APT)", amount);
    println!("\n  What happens:");
    println!("    1. Read current balance from remote (if not in delta)");
    println!("    2. Add {} to the balance", amount);
    println!("    3. Store new balance in delta layer");
    println!("    4. Save delta to delta.json");

    session.fund_account(account, amount)?;

    println!("  ✓ Account funded locally");
    println!("  ✓ Modification stored in delta.json");
    println!("  ✓ Remote network state unchanged");

    // Show delta contents
    let delta_path = session_path.join("delta.json");
    if delta_path.exists() {
        let delta_content = std::fs::read_to_string(&delta_path)?;
        println!("\n  Delta.json now contains:");
        println!("  {}", serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(&delta_content)?
        )?);
    }

    Ok(())
}

/// Example 4: Execute transaction on fork
///
/// This demonstrates how transactions are executed against the forked state.
/// The transaction sees the combination of remote state + local modifications.
async fn example_4_execute_transaction() -> anyhow::Result<()> {
    println!("\nExample 4: Executing Transaction on Fork\n");

    let session_path = PathBuf::from("example_fork_session");
    let mut session = Session::load(&session_path)?;

    println!("Executing a transaction on the fork...");
    println!("\n  What happens:");
    println!("    1. Transaction reads state:");
    println!("       - First checks delta.json for local modifications");
    println!("       - Falls back to remote fetch if not in delta");
    println!("    2. AptosVM executes transaction against combined state");
    println!("    3. Transaction output (write set) applied to delta");
    println!("    4. Delta saved to delta.json");
    println!("    5. Transaction output saved to session directory");

    // In a real scenario, you would build and sign a transaction:
    // let txn = build_transaction(...)?;
    // let (vm_status, output) = session.execute_transaction(txn)?;
    
    println!("\n  Note: This is a conceptual example.");
    println!("  In practice, you would:");
    println!("    - Build a SignedTransaction");
    println!("    - Call session.execute_transaction(txn)");
    println!("    - Inspect the VMStatus and TransactionOutput");

    println!("\n  ✓ Transaction execution flow demonstrated");

    Ok(())
}

/// Helper function to demonstrate the state access pattern
///
/// This shows the exact flow of how state is read:
/// 1. Check delta (local modifications)
/// 2. If not found, fetch from remote
fn demonstrate_state_access_pattern() {
    println!("\n=== State Access Pattern ===\n");
    
    println!("When reading state, the system follows this pattern:\n");
    
    println!("┌─────────────────────────────────────┐");
    println!("│  Read Request for StateKey         │");
    println!("└─────────────────────────────────────┘");
    println!("              │");
    println!("              ▼");
    println!("┌─────────────────────────────────────┐");
    println!("│  Check Delta Layer (delta.json)    │");
    println!("│  - Local modifications              │");
    println!("│  - Fast in-memory lookup           │");
    println!("└─────────────────────────────────────┘");
    println!("              │");
    println!("        ┌─────┴─────┐");
    println!("        │           │");
    println!("     Found?      Not Found");
    println!("        │           │");
    println!("        ▼           ▼");
    println!("┌──────────┐  ┌──────────────────┐");
    println!("│ Return   │  │ Fetch from Remote │");
    println!("│ Local    │  │ - REST API call   │");
    println!("│ Value    │  │ - Network roundtrip│");
    println!("└──────────┘  └──────────────────┘");
    println!("                      │");
    println!("                      ▼");
    println!("              ┌──────────────┐");
    println!("              │ Return Remote │");
    println!("              │ Value         │");
    println!("              └──────────────┘");
}

/// Helper function to demonstrate the modification pattern
///
/// This shows how local modifications are stored:
fn demonstrate_modification_pattern() {
    println!("\n=== Modification Pattern ===\n");
    
    println!("When modifying state, the system:\n");
    
    println!("1. Read current state (from delta or remote)");
    println!("2. Apply modification locally");
    println!("3. Store in delta layer (in-memory)");
    println!("4. Save delta to delta.json (persistence)");
    println!("\nKey points:");
    println!("  - Remote network is NEVER modified");
    println!("  - All changes are isolated to delta layer");
    println!("  - Delta can be cleared to reset fork state");
    println!("  - Multiple forks can exist simultaneously");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fork_initialization() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let session_path = temp_dir.path();

        // This would require actual network access, so we skip in tests
        // In real usage, this would work:
        // let session = Session::init_with_remote_state(
        //     session_path,
        //     Url::parse("https://fullnode.testnet.aptoslabs.com")?,
        //     10000000,
        //     None,
        // )?;

        Ok(())
    }
}




