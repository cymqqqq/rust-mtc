mod utils;
mod inscription;
mod bitcoin_tx;
mod wallet;
pub use wallet::address;
use utils::{init_ecdsa_public_key, read_public_key};
use ic_cdk::api::management_canister::bitcoin::{
    bitcoin_get_balance, bitcoin_get_current_fee_percentiles, BitcoinNetwork, GetBalanceRequest, GetCurrentFeePercentilesRequest, GetUtxosResponse, MillisatoshiPerByte
};
use ic_cdk_macros::{init, post_upgrade, pre_upgrade, update};
// use ic_management_canister_types::DerivationPath;
use utils::{ECDSAPublicKey, SendBtcRequest, UpdateUtxoRequest};
use wallet::{state, send_btc};
use std::cell::{Cell, RefCell};
use candid::candid_method;
use icrc_ledger_types::icrc1::account::Account;
use candid::Principal;


thread_local! {
    static NETWORK: Cell<BitcoinNetwork> = Cell::new(BitcoinNetwork::Testnet);

    // The derivation path to use for ECDSA secp256k1.
    static DERIVATION_PATH: Vec<Vec<u8>> = vec![];
    pub static SCHNORR_CANISTER: RefCell<String> = RefCell::new(String::from("6fwhw-fyaaa-aaaap-qb7ua-cai"));
    // 6fwhw-fyaaa-aaaap-qb7ua-cai
    // The ECDSA key name.
    static KEY_NAME: RefCell<String> = RefCell::new(String::from("test_key_1"));
}

#[init]
#[candid_method(init)]
pub async fn init(network: BitcoinNetwork) {
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
#[update]
#[candid_method(update)]
pub async fn init_pub_key() -> ECDSAPublicKey {
    init_ecdsa_public_key().await

}
/// Returns the balance of the given bitcoin address.
#[update]
#[candid_method(update)]
pub async fn get_balance(address: String) -> u64 {
    // let network = NETWORK.with(|n| n.get());
    match bitcoin_get_balance(GetBalanceRequest {network: BitcoinNetwork::Testnet, address, min_confirmations: Some(0)}).await {
        Ok(balance) => balance.0,
        Err(_) => 0u64
    }
}
/// Returns the UTXOs of the given bitcoin address.
#[update]
#[candid_method(update)]
pub async fn get_utxos() -> Vec<(String, u64)> {
    // let network = NETWORK.with(|n| n.get());
    // let mut utxo = Vec::new();
    state::read_wallet_utxo()
  
}
/// Returns the 100 fee percentiles measured in millisatoshi/byte.
/// Percentiles are computed from the last 10,000 transactions (if available).
#[update]
#[candid_method(update)]
pub async fn get_current_fee_percentiles() -> Vec<MillisatoshiPerByte> {
    
    // let network = NETWORK.with(|n| n.get());
    match bitcoin_get_current_fee_percentiles(GetCurrentFeePercentilesRequest{network: BitcoinNetwork::Testnet}).await {
        Ok(vec_byte) => vec_byte.0,
        Err(_) => vec![]
    }
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
    let pub_key = read_public_key().await;
    address::account_to_p2wpkh_address(network, &pub_key, &account).await
}


#[update]
#[candid_method(update)]
pub async fn get_p2pkh_address(pid: String) -> String {
    let principal = Principal::from_text(pid).expect("get principal from string failed");
    let account = Account {
        owner: principal,
        subaccount: None,
    };
    // let derivation_path = DERIVATION_PATH.with(|d| d.clone());
    // let key_name = KEY_NAME.with(|kn| kn.borrow().to_string());
    // let network = NETWORK.with(|n| n.get());
    let network = BitcoinNetwork::Testnet;
    let pub_key = read_public_key().await;
    address::account_to_p2pkh_address(network, &pub_key, &account).await
}
#[update]
#[candid_method(update)]
pub async fn send_btc(send_btc_request: SendBtcRequest) ->(Vec<u8>, String) {
    let dst_addr = send_btc_request.dst_address;
    let amount = send_btc_request.amount;
    let pid = Principal::from_text(send_btc_request.pid).unwrap();
    let account = Account { owner: pid, subaccount: None };
    let network = BitcoinNetwork::Testnet;
    // let key = read_public_key().await;
    let key_name = "test_key_1".to_string();
    let tx = send_btc::send(network, key_name, dst_addr, amount, &account).await;
    tx
}

#[update]
#[candid_method(update)]
pub async fn update_utxo(update_utxo_req: UpdateUtxoRequest) -> Vec<(String, u64)>{
    let network = BitcoinNetwork::Testnet;
    let address = update_utxo_req.address;
    state::update_utxo(network, address).await
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

