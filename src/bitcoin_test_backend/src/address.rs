use ic_cdk::api::management_canister::bitcoin::BitcoinNetwork;
use icrc_ledger_types::icrc1::account::Account;

use crate::{bitcoin_wallet::{derive_public_key, network_and_public_key_to_p2wpkh}, ecdsa_api::get_ecdsa_public_key, types::ECDSAPublicKey};
use crate::utils::*;
/// Derives a Bitcoin address for the specified account and converts it into
/// bech32 textual representation.
pub async fn account_to_p2wpkh_address(
    network: BitcoinNetwork,
    ecdsa_public_key: &ECDSAPublicKey,
    account: &Account,
) -> String {
    network_and_public_key_to_p2wpkh(
        network,
        &derive_public_key(&ecdsa_public_key, account).public_key,
    )
}

/// Returns the P2PKH address of this canister at the given derivation path.
pub async fn get_p2pkh_address(
    network: BitcoinNetwork,
    ecdsa_public_key: &ECDSAPublicKey,
    account: &Account,
) -> String {
    network_and_public_key_to_p2pkh(
        network,
        &derive_public_key(&ecdsa_public_key, account).public_key,
    )
    // // Compute the address.
    // public_key_to_p2pkh_address(network, &public_key)
}

pub fn network_and_public_key_to_p2pkh(network: BitcoinNetwork, public_key: &[u8]) -> String {
    // assert_eq!(public_key.len(), 33);
    // assert!(public_key[0] == 0x02 || public_key[0] == 0x03);
    public_key_to_p2pkh_address(network, public_key)
}

// Converts a public key to a P2PKH address.
fn public_key_to_p2pkh_address(network: BitcoinNetwork, public_key: &[u8]) -> String {
    // SHA-256 & RIPEMD-160
    let result = ripemd160(&sha256(public_key));

    let prefix = match network {
        BitcoinNetwork::Testnet | BitcoinNetwork::Regtest => 0x6f,
        BitcoinNetwork::Mainnet => 0x00,
    };
    let mut data_with_prefix = vec![prefix];
    data_with_prefix.extend(result);

    let checksum = &sha256(&sha256(&data_with_prefix.clone()))[..4];

    let mut full_address = data_with_prefix;
    full_address.extend(checksum);

    bs58::encode(full_address).into_string()
}

