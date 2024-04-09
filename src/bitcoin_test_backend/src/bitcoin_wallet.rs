//! A demo of a very bare-bones bitcoin "wallet".
//!
//! The wallet here showcases how bitcoin addresses can be be computed
//! and how bitcoin transactions can be signed. It is missing several
//! pieces that any production-grade wallet would have, including:
//!
//! * Support for address types that aren't P2PKH.
//! * Caching spent UTXOs so that they are not reused in future transactions.
//! * Option to set the fee.
use std::str::FromStr;

use crate::{bitcoin_api::{self, JsonOutPoint}, ecdsa_api::{self, sign_with_ecdsa}, types::ECDSAPublicKeyReply};
use bitcoin::{
    absolute::LockTime, bech32, blockdata::{script::Builder, witness::Witness}, consensus::serialize, hashes::Hash, script::{PushBytes, PushBytesBuf}, sighash::{EcdsaSighashType, SegwitV0Sighash, SighashCache}, Address, AddressType, Network, OutPoint, Script, ScriptBuf, Transaction, TxIn, TxOut, Txid
};
use ic_cdk::{api::management_canister::bitcoin::{BitcoinAddress, BitcoinNetwork, MillisatoshiPerByte, Satoshi, Utxo}, eprintln};
use icrc_ledger_types::icrc1::account::Account;
use sha2::{Digest, Sha256};
use serde_bytes::ByteBuf;
use ic_crypto_extended_bip32::{DerivationIndex, DerivationPath, ExtendedBip32DerivationOutput};
// use crate::ecdsa_api::{DerivationPath};
const SIG_HASH_TYPE: EcdsaSighashType = EcdsaSighashType::All;

/// Returns the P2PKH address of this canister at the given derivation path.
// pub async fn get_p2pkh_address(
//     network: BitcoinNetwork,
//     key_name: String,
//     derivation_path: Vec<Vec<u8>>,
// ) -> String {
//     // Fetch the public key of the given derivation path.
//     let public_key = ecdsa_api::ecdsa_public_key(key_name, derivation_path).await;

//     // Compute the address.
//     public_key_to_p2pkh_address(network, &public_key)
// }

/// Returns the derivation path that should be used to sign a message from a
/// specified account.
pub fn derivation_path(account: &Account) -> Vec<ByteBuf> {
    const SCHEMA_V1: u8 = 1;
    vec![
        ByteBuf::from(vec![SCHEMA_V1]),
        ByteBuf::from(account.owner.as_slice().to_vec()),
        ByteBuf::from(account.effective_subaccount().to_vec()),
    ]
}

/// Returns a valid extended BIP-32 derivation path from an Account (Principal + subaccount)
pub fn derive_public_key(ecdsa_public_key: &ECDSAPublicKeyReply, account: &Account) -> ECDSAPublicKeyReply {
    let ExtendedBip32DerivationOutput {
        derived_public_key,
        derived_chain_code,
    } = DerivationPath::new(
        derivation_path(account)
            .into_iter()
            .map(|x| DerivationIndex(x.into_vec()))
            .collect(),
    )
    .public_key_derivation(&ecdsa_public_key.public_key, &ecdsa_public_key.chain_code)
    .expect("bug: failed to derive an ECDSA public key from valid inputs");
    ECDSAPublicKeyReply {
        public_key: derived_public_key,
        chain_code: derived_chain_code,
    }
}


/// Derives a Bitcoin address for the specified account and converts it into
/// bech32 textual representation.
pub async fn account_to_p2wpkh_address(
    network: BitcoinNetwork,
    // ecdsa_public_key: &ECDSAPublicKeyReply,
    key_name: String,
    account: &Account,
    // derivation_path: Vec<Vec<u8>>,
) -> String {
    let ecdsa_public_key =  ecdsa_api::ecdsa_public_key(key_name.clone(), vec!["m/86/0/0/0".as_bytes().to_vec()]).await;

    network_and_public_key_to_p2wpkh(
        network,
        &derive_public_key(&ecdsa_public_key, account).public_key,
    )
}

/// Sends a transaction to the network that transfers the given amount to the
/// given destination, where the source of the funds is the canister itself
/// at the given derivation path.
pub async fn send(
    network: BitcoinNetwork,
    path: Vec<Vec<u8>>,
    key_name: String,
    dst_address: String,
    amount: Satoshi,
    account: &Account
) -> Txid {
    // Get fee percentiles from previous transactions to estimate our own fee.
    let fee_percentiles = bitcoin_api::get_current_fee_percent(network).await;

    let fee_per_byte = if fee_percentiles.is_empty() {
        // There are no fee percentiles. This case can only happen on a regtest
        // network where there are no non-coinbase transactions. In this case,
        // we use a default of 2000 millisatoshis/byte (i.e. 2 satoshi/byte)
        2000
    } else {
        // Choose the 50th percentile for sending fees.
        fee_percentiles[50]
    };

    // Fetch our public key, P2PKH address, and UTXOs.
    let own_public_key =
        ecdsa_api::ecdsa_public_key(key_name.clone(), path.clone()).await;
    // let own_address = public_key_to_p2pkh_address(network, &own_public_key);
    // let own_address = account_to_p2wpkh_address(network, key_name.clone(),&account).await;
    let own_address = "tb1qs0y2rvapywv9pxdzjxmcn4gx8yhuf4kq3yv5qy".to_string();
    // print("Fetching UTXOs...");
    // Note that pagination may have to be used to get all UTXOs for the given address.
    // For the sake of simplicity, it is assumed here that the `utxo` field in the response
    // contains all UTXOs.
    let own_utxos = bitcoin_api::get_all_utxo_from_wallet();

    let own_address = Address::from_str(&own_address).unwrap().require_network(Network::Testnet).unwrap();
    let dst_address = Address::from_str(&dst_address).unwrap().require_network(Network::Testnet).unwrap();

    // Build the transaction that sends `amount` to the destination address.
    let transaction = build_transaction(
        &own_public_key,
        &own_address,
        &own_utxos,
        &dst_address,
        amount,
        fee_per_byte,
        account
    )
    .await;

    // let tx_bytes = serialize(&transaction);
    // print(&format!("Transaction to sign: {}", hex::encode(tx_bytes)));

    // Sign the transaction.
    let signed_transaction = sign_transaction(
        &own_public_key,
        &own_address,
        transaction,
        key_name,
        path,
        amount,
        account,
    )
    .await;

    let signed_transaction_bytes = serialize(&signed_transaction);
    // print(&format!(
    //     "Signed transaction: {}",
    //     hex::encode(&signed_transaction_bytes)
    // ));

    // print("Sending transaction...");
    bitcoin_api::send_transaction(network, signed_transaction_bytes).await;
    // print("Done");

    signed_transaction.txid()
}

// Builds a transaction to send the given `amount` of satoshis to the
// destination address.
async fn build_transaction(
    own_public_key: &ECDSAPublicKeyReply,
    own_address: &Address,
    own_utxos: &Vec<(JsonOutPoint, u64)>,
    dst_address: &Address,
    amount: Satoshi,
    fee_per_byte: MillisatoshiPerByte,
    account: &Account
) -> Transaction {
    // We have a chicken-and-egg problem where we need to know the length
    // of the transaction in order to compute its proper fee, but we need
    // to know the proper fee in order to figure out the inputs needed for
    // the transaction.
    //
    // We solve this problem iteratively. We start with a fee of zero, build
    // and sign a transaction, see what its size is, and then update the fee,
    // rebuild the transaction, until the fee is set to the correct amount.
    // print("Building transaction...");
    let mut total_fee = 0;
    loop {
        let transaction =
            build_transaction_with_fee(own_utxos, own_address, dst_address, amount, total_fee)
                .expect("Error building transaction.");

        // Sign the transaction. In this case, we only care about the size
        // of the signed transaction, so we use a mock signer here for efficiency.
        let signed_transaction = sign_transaction(
            own_public_key,
            own_address,
            transaction.clone(),
            "test_key_1".to_string(), // mock key name
            vec![],    
            amount,       // mock derivation path
            account,
        )
        .await;

        let signed_tx_bytes_len = serialize(&signed_transaction).len() as u64;

        if (signed_tx_bytes_len * fee_per_byte) / 1000 == total_fee {
            // print(&format!("Transaction built with fee {}.", total_fee));
            return transaction;
        } else {
            total_fee = (signed_tx_bytes_len * fee_per_byte) / 1000;
        }
    }
}

fn build_transaction_with_fee(
    own_utxos: &Vec<(JsonOutPoint, u64)>,
    own_address: &Address,
    dst_address: &Address,
    amount: u64,
    fee: u64,
) -> Result<Transaction, String> {
    // Assume that any amount below this threshold is dust.
    const DUST_THRESHOLD: u64 = 1_000;

    // Select which UTXOs to spend. We naively spend the oldest available UTXOs,
    // even if they were previously spent in a transaction. This isn't a
    // problem as long as at most one transaction is created per block and
    // we're using min_confirmations of 1.
    let mut utxos_to_spend = vec![];
    let mut total_spent = 0;
    for (outpoint, amount) in own_utxos.iter().rev() {
        total_spent += amount;
        utxos_to_spend.push((outpoint, amount));
        if total_spent >= amount + fee {
            // We have enough inputs to cover the amount we want to spend.
            break;
        }
    }

    if total_spent < amount + fee {
        return Err(format!(
            "Insufficient balance: {}, trying to transfer {} satoshi with fee {}",
            total_spent, amount, fee
        ));
    }

    let inputs: Vec<TxIn> = utxos_to_spend
        .into_iter()
        .map(|(utxo, _)| TxIn {
            previous_output: OutPoint {
                txid: Txid::from_slice(utxo.txid()).unwrap(),
                //  Txid::from_raw_hash(Hash::from_slice(&utxo.txid()).unwrap()),
                vout: utxo.vout(),
            },
            sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::new(),
            script_sig: ScriptBuf::new(),
        })
        .collect();

    let mut outputs = vec![TxOut {
        script_pubkey: dst_address.script_pubkey(),
        value: amount,
    }];

    let remaining_amount = total_spent - amount - fee;

    if remaining_amount >= DUST_THRESHOLD {
        outputs.push(TxOut {
            script_pubkey: own_address.script_pubkey(),
            value: remaining_amount,
        });
    }

    Ok(Transaction {
        input: inputs,
        output: outputs,
        lock_time: LockTime::from_time(0).unwrap(),
        version: 1,
    })
}

// Sign a bitcoin transaction.
//
// IMPORTANT: This method is for demonstration purposes only and it only
// supports signing transactions if:
//
// 1. All the inputs are referencing outpoints that are owned by `own_address`.
// 2. `own_address` is a P2PKH address.
async fn sign_transaction
// <SignFun, Fut>
(
    own_public_key: &ECDSAPublicKeyReply,
    own_address: &Address,
    mut transaction: Transaction,
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
    amount: Satoshi,
    account: &Account,
    // signer: SignFun,
) -> Transaction
// where
//     SignFun: Fn(String, Vec<Vec<u8>>, Vec<u8>) -> Fut,
//     Fut: std::future::Future<Output = Vec<u8>>,
{
    // Verify that our own address is P2PKH.
    assert_eq!(
        own_address.address_type(),
        Some(AddressType::P2wpkh),
        "This example supports signing p2wpkh addresses only."
    );
    let mut sighashcache = SighashCache::new(transaction.clone());
    // let txclone = transaction.clone();
    let pubkey = ByteBuf::from(derive_public_key(own_public_key, account).public_key);
    let pkhash = hash160(&pubkey);
    // let path = deri(account);
    for (index, input) in transaction.input.iter_mut().enumerate() {
        // let sighash =
        //     txclone.signature_hash(index, &own_address.script_pubkey(), SIG_HASH_TYPE.to_u32());
        let sighash = sighashcache.segwit_signature_hash(index, &Script::from_bytes(&pkhash), amount, EcdsaSighashType::All).expect("build sighash failed");
        let signature =
        sign_with_ecdsa(key_name.clone(), &derivation_path, sighash.to_byte_array().to_vec())
            .await;
        // let signature = signer(key_name.clone(), derivation_path.clone(), sighash.to_byte_array().to_vec()).await;

        // Convert signature to DER.
        let der_signature = sec1_to_der(signature);

        let mut sig_with_hashtype = der_signature;
        sig_with_hashtype.push(SIG_HASH_TYPE.to_u32() as u8);
        let sig_data = PushBytesBuf::try_from(sig_with_hashtype.as_slice().to_vec()).unwrap();
        let pubkey_data = PushBytesBuf::try_from(own_public_key.public_key.to_vec()).unwrap();
        input.script_sig = Builder::new()
            .push_slice(sig_data)
            .push_slice(pubkey_data)
            .into_script();
        input.witness.clear();
    }

    transaction
}

fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
fn ripemd160(data: &[u8]) -> Vec<u8> {
    let mut hasher = ripemd::Ripemd160::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// SHA-256 followed by Ripemd160, also known as HASH160.
pub fn hash160(bytes: &[u8]) -> [u8; 20] {
    use ripemd::{Digest, Ripemd160};
    Ripemd160::digest(Sha256::digest(bytes)).into()
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum WitnessVersion {
    V0 = 0,
    V1 = 1,
}

/// Calculates the p2wpkh address as described in [BIP-0173](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki).
///
/// # Panics
///
/// This function panics if the public key in not compressed.
pub fn network_and_public_key_to_p2wpkh(network: BitcoinNetwork, public_key: &[u8]) -> String {
    assert_eq!(public_key.len(), 33);
    assert!(public_key[0] == 0x02 || public_key[0] == 0x03);
    encode_bech32(network, &ripemd160(&sha256(public_key)), WitnessVersion::V0)
}

/// Returns the human-readable part of a bech32 address
pub fn hrp(network: BitcoinNetwork) -> &'static str {
    match network {
        BitcoinNetwork::Mainnet => "bc",
        BitcoinNetwork::Testnet => "tb",
        BitcoinNetwork::Regtest => "bcrt",
        _ => todo!(),
    }
}
fn encode_bech32(network: BitcoinNetwork, hash: &[u8], version: WitnessVersion) -> String {
    use bech32::u5;

    let hrp = hrp(network);
    let witness_version: u5 =
        u5::try_from_u8(version as u8).expect("bug: witness version must be smaller than 32");
    let data: Vec<u5> = std::iter::once(witness_version)
        .chain(
            bech32::convert_bits(hash, 8, 5, true)
                .expect("bug: bech32 bit conversion failed on valid inputs")
                .into_iter()
                .map(|b| {
                    u5::try_from_u8(b).expect("bug: bech32 bit conversion produced invalid outputs")
                }),
        )
        .collect();
    match version {
        WitnessVersion::V0 => bech32::encode(hrp, data, bech32::Variant::Bech32)
            .expect("bug: bech32 encoding failed on valid inputs"),
        WitnessVersion::V1 => bech32::encode(hrp, data, bech32::Variant::Bech32m)
            .expect("bug: bech32m encoding failed on valid inputs"),
    }
}

// // A mock for rubber-stamping ECDSA signatures.
// async fn mock_signer(
//     _key_name: String,
//     _derivation_path: Vec<Vec<u8>>,
//     _message_hash: Vec<u8>,
// ) -> Vec<u8> {
//     vec![255; 64]
// }

// Converts a SEC1 ECDSA signature to the DER format.
fn sec1_to_der(sec1_signature: Vec<u8>) -> Vec<u8> {
    let r: Vec<u8> = if sec1_signature[0] & 0x80 != 0 {
        // r is negative. Prepend a zero byte.
        let mut tmp = vec![0x00];
        tmp.extend(sec1_signature[..32].to_vec());
        tmp
    } else {
        // r is positive.
        sec1_signature[..32].to_vec()
    };

    let s: Vec<u8> = if sec1_signature[32] & 0x80 != 0 {
        // s is negative. Prepend a zero byte.
        let mut tmp = vec![0x00];
        tmp.extend(sec1_signature[32..].to_vec());
        tmp
    } else {
        // s is positive.
        sec1_signature[32..].to_vec()
    };

    // Convert signature to DER.
    vec![
        vec![0x30, 4 + r.len() as u8 + s.len() as u8, 0x02, r.len() as u8],
        r,
        vec![0x02, s.len() as u8],
        s,
    ]
    .into_iter()
    .flatten()
    .collect()
}