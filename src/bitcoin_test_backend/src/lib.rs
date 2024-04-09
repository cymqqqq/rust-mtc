mod types;
mod ecdsa_api;
mod bitcoin_api;
mod bitcoin_wallet;

use bitcoin::Network;
use bitcoin_api::JsonOutPoint;
use ic_cdk::api::management_canister::bitcoin::{
    BitcoinNetwork, GetUtxosResponse, MillisatoshiPerByte
};
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, update};
use std::cell::{Cell, RefCell};
use candid::candid_method;
use icrc_ledger_types::icrc1::account::Account;
use candid::Principal;


thread_local! {
    static NETWORK: Cell<BitcoinNetwork> = Cell::new(BitcoinNetwork::Testnet);

    // The derivation path to use for ECDSA secp256k1.
    static DERIVATION_PATH: Vec<Vec<u8>> = vec![];

    // The ECDSA key name.
    static KEY_NAME: RefCell<String> = RefCell::new(String::from("test_key_1"));
}

#[init]
#[candid_method(init)]
pub fn init(network: BitcoinNetwork) {
    NETWORK.with(|nw| nw.set(network));
    KEY_NAME.with(|key_name| {
        key_name.replace(String::from(match network {
            // For local development, we use a special test key with dfx.
            BitcoinNetwork::Regtest => "dfx_test_key",
            // On the IC we're using a test ECDSA key.
            BitcoinNetwork::Mainnet | BitcoinNetwork::Testnet => "test_key_1",
        }))
    });
}

/// Returns the balance of the given bitcoin address.
#[update]
#[candid_method(update)]
pub async fn get_balance(address: String) -> u64 {
    // let network = NETWORK.with(|n| n.get());
    bitcoin_api::get_balance(BitcoinNetwork::Testnet, address).await
}

/// Returns the UTXOs of the given bitcoin address.
#[update]
#[candid_method(update)]
pub async fn get_utxos() -> Vec<(String, u64)> {
    // let network = NETWORK.with(|n| n.get());
    // let mut utxo = Vec::new();
    bitcoin_api::read_wallet_utxo()
  
}
/// Returns the 100 fee percentiles measured in millisatoshi/byte.
/// Percentiles are computed from the last 10,000 transactions (if available).
#[update]
#[candid_method(update)]
pub async fn get_current_fee_percentiles() -> Vec<MillisatoshiPerByte> {
    // let network = NETWORK.with(|n| n.get());
    bitcoin_api::get_current_fee_percent(BitcoinNetwork::Testnet).await
}

/// Returns the P2PKH address of this canister at a specific derivation path.
// #[update]
// #[candid_method(update)]
// pub async fn get_p2pkh_address() -> String {
//     let derivation_path = DERIVATION_PATH.with(|d| d.clone());
//     // let key_name = KEY_NAME.with(|kn| kn.borrow().to_string());
//     // let network = NETWORK.with(|n| n.get());
//     bitcoin_wallet::get_p2pkh_address(BitcoinNetwork::Testnet, "test_key_1".to_string(), derivation_path).await
// }

#[update]
#[candid_method(update)]
pub async fn get_p2wpkh_address(pid: String) -> String {
    let principal = Principal::from_text(pid).expect("get principal from string failed");
    let account = Account {
        owner: principal,
        subaccount: None,
    };
    // let derivation_path = DERIVATION_PATH.with(|d| d.clone());
    // let key_name = KEY_NAME.with(|kn| kn.borrow().to_string());
    // let network = NETWORK.with(|n| n.get());
    let network = BitcoinNetwork::Testnet;
    bitcoin_wallet::account_to_p2wpkh_address(network, "test_key_1".to_string(), &account).await
}


// #[pre_upgrade]
// fn pre_upgrade() {
//     let network = NETWORK.with(|n| n.get());
//     ic_cdk::storage::stable_save((network,)).expect("Saving network to stable store must succeed.");
// }

// #[post_upgrade]
// fn post_upgrade() {
//     let network = ic_cdk::storage::stable_restore::<(BitcoinNetwork,)>()
//         .expect("Failed to read network from stable memory.")
//         .0;

//     init(network);
// }

