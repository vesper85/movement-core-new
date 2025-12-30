// Copyright (c) Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use crate::{
    common::types::{
        CliCommand, CliError, CliResult, CliTypedResult, EntryFunctionArguments, TransactionSummary,
    },
    move_tool::ReplayNetworkSelection,
};
use aptos_crypto::{ed25519::Ed25519PrivateKey, Uniform};
use aptos_rest_client::{AptosBaseUrl, Client};
use aptos_transaction_simulation_session::Session;
use aptos_types::transaction::{
    EntryFunction, RawTransaction, SignedTransaction, TransactionPayload,
};
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use move_core_types::{account_address::AccountAddress, language_storage::StructTag};
use std::path::PathBuf;
use url::Url;

/// Convert ReplayNetworkSelection to a URL
fn network_to_url(network: &ReplayNetworkSelection) -> Result<Url, crate::common::types::CliError> {
    match network {
        ReplayNetworkSelection::Mainnet => Url::parse("https://mainnet.aptoslabs.com")
            .map_err(|e| crate::common::types::CliError::UnexpectedError(e.to_string())),
        ReplayNetworkSelection::Testnet => Url::parse("https://testnet.aptoslabs.com")
            .map_err(|e| crate::common::types::CliError::UnexpectedError(e.to_string())),
        ReplayNetworkSelection::Devnet => Url::parse("https://devnet.aptoslabs.com")
            .map_err(|e| crate::common::types::CliError::UnexpectedError(e.to_string())),
        ReplayNetworkSelection::RestEndpoint(url_str) => Url::parse(url_str)
            .map_err(|e| crate::common::types::CliError::UnexpectedError(e.to_string())),
    }
}

/// Initializes a new simulation session
#[derive(Debug, Parser)]
pub struct Init {
    /// Path to the directory where the session data will be stored.
    #[clap(long)]
    path: PathBuf,

    /// If specified, starts the simulation by forking from a remote network state.
    #[clap(long)]
    network: Option<ReplayNetworkSelection>,

    /// The version of the network state to fork from.
    ///
    /// Only used if `--network` is specified.
    ///
    /// If not specified, the latest version of the network will be used.
    #[clap(long)]
    network_version: Option<u64>,

    /// API key for connecting to the fullnode.
    ///
    /// It is strongly recommended to specify an API key to avoid rate limiting.
    #[clap(long)]
    api_key: Option<String>,
}

#[async_trait]
impl CliCommand<()> for Init {
    fn command_name(&self) -> &'static str {
        "init"
    }

    async fn execute(self) -> CliTypedResult<()> {
        match self.network {
            Some(network) => {
                let url = network_to_url(&network)?;
                let network_version = match self.network_version {
                    Some(txn_id) => txn_id,
                    None => {
                        let client = Client::builder(AptosBaseUrl::Custom(url.clone())).build();
                        client.get_ledger_information().await?.inner().version
                    },
                };

                Session::init_with_remote_state(&self.path, url, network_version, self.api_key)?;
            },
            None => {
                Session::init(&self.path)?;
            },
        }

        Ok(())
    }
}

/// Funds an account with APT tokens
#[derive(Debug, Parser)]
pub struct Fund {
    /// Path to a stored session
    #[clap(long)]
    session: PathBuf,

    /// Account to fund, can be an address or a CLI profile name
    #[clap(long, value_parser = crate::common::types::load_account_arg)]
    account: AccountAddress,

    /// Funding amount, in Octa (10^-8 APT)
    #[clap(long)]
    amount: u64,

    /// Optional public key for the account (if not provided, reads from session profile)
    #[clap(long)]
    public_key: Option<String>,
}

#[async_trait]
impl CliCommand<()> for Fund {
    fn command_name(&self) -> &'static str {
        "fund"
    }

    async fn execute(self) -> CliTypedResult<()> {
        let mut session = Session::load(&self.session)?;

        // Try to get public key from argument or session profile
        let public_key_opt = if let Some(pk_str) = &self.public_key {
            // Parse public key from argument (handle both old and new format)
            let pk_hex = pk_str
                .trim_start_matches("ed25519-pub-")
                .trim_start_matches("0x");
            let pk_bytes = hex::decode(pk_hex)
                .map_err(|e| CliError::UnexpectedError(format!("Invalid public key hex: {}", e)))?;
            Some(
                aptos_crypto::ed25519::Ed25519PublicKey::try_from(pk_bytes.as_slice())
                    .map_err(|e| CliError::UnexpectedError(format!("Invalid public key: {}", e)))?,
            )
        } else {
            // Try to read from session's profile
            let config_path = self.session.join(".movement").join("config.yaml");
            if config_path.exists() {
                let config_content = std::fs::read_to_string(&config_path)
                    .map_err(|e| CliError::IO(format!("Failed to read config: {}", e), e))?;

                config_content
                    .lines()
                    .find(|line| line.trim().starts_with("public_key:"))
                    .and_then(|line| {
                        let parts: Vec<&str> = line.split(':').collect();
                        if parts.len() >= 2 {
                            Some(parts[1..].join(":").trim().trim_matches('"').to_string())
                        } else {
                            None
                        }
                    })
                    .and_then(|pk_str| {
                        let pk_hex = pk_str
                            .trim_start_matches("ed25519-pub-")
                            .trim_start_matches("0x");
                        hex::decode(pk_hex).ok()
                    })
                    .and_then(|pk_bytes| {
                        aptos_crypto::ed25519::Ed25519PublicKey::try_from(pk_bytes.as_slice()).ok()
                    })
            } else {
                None
            }
        };

        // Use create_and_fund_account if we have a public key, otherwise fallback to fund_account
        match public_key_opt {
            Some(public_key) => {
                session.create_and_fund_account(self.account, public_key, self.amount)?;
            },
            None => {
                // Fallback to old behavior (just fund fungible store)
                session.fund_account(self.account, self.amount)?;
            },
        }

        Ok(())
    }
}

/// View a resource
#[derive(Debug, Parser)]
pub struct ViewResource {
    /// Path to a stored session
    #[clap(long)]
    session: PathBuf,

    /// Account under which the resource is stored
    #[clap(long, value_parser = crate::common::types::load_account_arg)]
    account: AccountAddress,

    /// Resource to view
    #[clap(long)]
    resource: StructTag,
}

#[async_trait]
impl CliCommand<Option<serde_json::Value>> for ViewResource {
    fn command_name(&self) -> &'static str {
        "view-resource"
    }

    async fn execute(self) -> CliTypedResult<Option<serde_json::Value>> {
        let mut session = Session::load(&self.session)?;

        Ok(session.view_resource(self.account, &self.resource)?)
    }
}

/// View a resource group
#[derive(Debug, Parser)]
pub struct ViewResourceGroup {
    /// Path to a stored session
    #[clap(long)]
    session: PathBuf,

    /// Account under which the resource group is stored
    #[clap(long, value_parser = crate::common::types::load_account_arg)]
    account: AccountAddress,

    /// Resource group to view
    #[clap(long)]
    resource_group: StructTag,

    /// If specified, derives an object address from the source address and an object
    #[clap(long)]
    derived_object_address: Option<AccountAddress>,
}

#[async_trait]
impl CliCommand<Option<serde_json::Value>> for ViewResourceGroup {
    fn command_name(&self) -> &'static str {
        "view-resource-group"
    }

    async fn execute(self) -> CliTypedResult<Option<serde_json::Value>> {
        let mut session = Session::load(&self.session)?;
        Ok(session.view_resource_group(
            self.account,
            &self.resource_group,
            self.derived_object_address,
        )?)
    }
}

/// Run an entry function transaction against a simulation session
#[derive(Debug, Parser)]
pub struct Run {
    /// Path to a stored session
    #[clap(long)]
    session: PathBuf,

    #[clap(flatten)]
    pub(crate) entry_function_args: EntryFunctionArguments,

    /// Sender account address (optional, uses random if not provided)
    #[clap(long, value_parser = crate::common::types::load_account_arg)]
    sender_account: Option<AccountAddress>,

    /// Maximum gas units willing to pay
    #[clap(long, default_value = "100000")]
    max_gas: u64,

    /// Gas unit price in octas
    #[clap(long, default_value = "100")]
    gas_unit_price: u64,
}

#[async_trait]
impl CliCommand<TransactionSummary> for Run {
    fn command_name(&self) -> &'static str {
        "run"
    }

    async fn execute(self) -> CliTypedResult<TransactionSummary> {
        let mut session = Session::load(&self.session)?;

        // Try to load the profile from the session directory
        let config_path = self.session.join(".movement").join("config.yaml");
        let (private_key, sender) = if config_path.exists() {
            // Parse the YAML config to get the private key and account
            let config_content = std::fs::read_to_string(&config_path)
                .map_err(|e| CliError::IO(format!("Failed to read config: {}", e), e))?;

            // Simple YAML parsing for the default profile
            let private_key_str = config_content
                .lines()
                .find(|line| line.trim().starts_with("private_key:"))
                .and_then(|line| {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 2 {
                        Some(parts[1..].join(":").trim().trim_matches('"').to_string())
                    } else {
                        None
                    }
                });

            let account_str = config_content
                .lines()
                .find(|line| line.trim().starts_with("account:"))
                .and_then(|line| {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 2 {
                        Some(parts[1..].join(":").trim().trim_matches('"').to_string())
                    } else {
                        None
                    }
                });

            if let (Some(pk_str), Some(acc_str)) = (private_key_str, account_str) {
                // Handle both old format (0x...) and new format (ed25519-priv-0x...)
                let pk_hex = pk_str
                    .trim_start_matches("ed25519-priv-")
                    .trim_start_matches("0x");
                let private_key = Ed25519PrivateKey::try_from(
                    hex::decode(pk_hex)
                        .map_err(|e| {
                            CliError::UnexpectedError(format!("Invalid private key hex: {}", e))
                        })?
                        .as_slice(),
                )
                .map_err(|e| CliError::UnexpectedError(format!("Invalid private key: {}", e)))?;

                let sender_address = AccountAddress::from_hex_literal(&format!(
                    "0x{}",
                    acc_str.trim_start_matches("0x")
                ))
                .map_err(|e| {
                    CliError::UnexpectedError(format!("Invalid account address: {}", e))
                })?;

                (private_key, sender_address)
            } else {
                // Fallback to random if profile parsing fails
                let private_key = Ed25519PrivateKey::generate(&mut rand::thread_rng());
                let sender = self.sender_account.unwrap_or_else(AccountAddress::random);
                (private_key, sender)
            }
        } else {
            // No profile, use random or provided sender
            let private_key = Ed25519PrivateKey::generate(&mut rand::thread_rng());
            let sender = self.sender_account.unwrap_or_else(AccountAddress::random);
            (private_key, sender)
        };

        // Use provided sender if specified, otherwise use profile's account
        let sender = self.sender_account.unwrap_or(sender);

        // Parse entry function arguments
        let entry_function: EntryFunction = self.entry_function_args.try_into()?;

        // Create payload
        let payload = TransactionPayload::EntryFunction(entry_function);

        // Get the current sequence number for the sender from the session state
        let sequence_number = session.get_sequence_number(sender).unwrap_or(0);

        // Create raw transaction
        let raw_txn = RawTransaction::new(
            sender,
            sequence_number,
            payload,
            self.max_gas,
            self.gas_unit_price,
            chrono::Utc::now().timestamp() as u64 + 600, // expiration 10 minutes from now
            aptos_types::chain_id::ChainId::new(126),    // Movement mainnet chain ID
        );

        // Sign the transaction with the profile's private key
        let signed_txn = SignedTransaction::new(
            raw_txn,
            aptos_crypto::ed25519::Ed25519PublicKey::from(&private_key),
            aptos_crypto::ed25519::Ed25519Signature::try_from(&[0u8; 64][..])
                .expect("Invalid signature bytes"),
        );

        // Execute the transaction
        let (vm_status, txn_output) = session.execute_transaction(signed_txn).map_err(|e| {
            CliError::UnexpectedError(format!("Transaction execution failed: {}", e))
        })?;

        // Create transaction summary
        let success = match txn_output.status() {
            aptos_types::transaction::TransactionStatus::Keep(exec_status) => {
                Some(exec_status.is_success())
            },
            aptos_types::transaction::TransactionStatus::Discard(_)
            | aptos_types::transaction::TransactionStatus::Retry => None,
        };

        Ok(TransactionSummary {
            transaction_hash: aptos_crypto::HashValue::zero().into(),
            gas_used: Some(txn_output.gas_used()),
            gas_unit_price: Some(self.gas_unit_price),
            pending: None,
            sender: Some(sender),
            sequence_number: Some(0),
            success,
            timestamp_us: None,
            version: None,
            vm_status: Some(vm_status.to_string()),
        })
    }
}

/// BETA: Commands for interacting with a local simulation session
///
/// BETA: Subject to change
#[derive(Subcommand)]
pub enum Sim {
    Init(Init),
    Fund(Fund),
    Run(Run),
    ViewResource(ViewResource),
    ViewResourceGroup(ViewResourceGroup),
}

impl Sim {
    pub async fn execute(self) -> CliResult {
        match self {
            Sim::Init(init) => init.execute_serialized_success().await,
            Sim::Fund(fund) => fund.execute_serialized_success().await,
            Sim::Run(run) => run.execute_serialized().await,
            Sim::ViewResource(view_resource) => view_resource.execute_serialized().await,
            Sim::ViewResourceGroup(view_resource_group) => {
                view_resource_group.execute_serialized().await
            },
        }
    }
}
