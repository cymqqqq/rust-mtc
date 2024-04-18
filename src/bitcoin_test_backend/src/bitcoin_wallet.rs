//! A demo of a very bare-bones bitcoin "wallet".
//!
//! The wallet here showcases how bitcoin addresses can be be computed
//! and how bitcoin transactions can be signed. It is missing several
//! pieces that any production-grade wallet would have, including:
//!
//! * Support for address types that aren't P2PKH.
//! * Caching spent UTXOs so that they are not reused in future transactions.
//! * Option to set the fee.
use std::{collections::HashMap, str::FromStr};

use crate::{
    bitcoin_api::{self, JsonOutPoint}, 
    ecdsa_api::{read_public_key, get_sign_with_ecdsa}, 
    types::ECDSAPublicKey
};
use bitcoin::{
    absolute::LockTime, 
    blockdata::{script::Builder, witness::Witness}, 
    consensus::serialize, 
    ecdsa::Signature, 
    hashes::Hash, 
    script::PushBytesBuf, 
    sighash::{EcdsaSighashType, SighashCache}, 
    transaction::Version, 
    Address, 
    AddressType, 
    Amount, 
    CompressedPublicKey, 
    Network, 
    OutPoint,
    Script,  
    Transaction, 
    TxIn, 
    TxOut, 
    Txid
};
use ic_cdk::
    api::management_canister::{bitcoin::{
        bitcoin_get_current_fee_percentiles, 
        bitcoin_send_transaction, 
        BitcoinNetwork, 
        GetCurrentFeePercentilesRequest, 
        MillisatoshiPerByte, 
        Satoshi, 
        SendTransactionRequest, 
}, ecdsa::SignWithEcdsaResponse};
use icrc_ledger_types::icrc1::account::Account;
use serde_bytes::ByteBuf;
use ic_crypto_extended_bip32::{DerivationPath, DerivationIndex, ExtendedBip32DerivationOutput};
use ic_management_canister_types::ECDSAPublicKeyResponse;
use crate::utils::*;
// use crate::ecdsa_api::{DerivationPath};
const SIG_HASH_TYPE: EcdsaSighashType = EcdsaSighashType::All;





/// Returns a valid extended BIP-32 derivation path from an Account (Principal + subaccount)
pub fn derive_public_key(ecdsa_public_key: &ECDSAPublicKey, account: &Account) -> ECDSAPublicKeyResponse {
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
    ECDSAPublicKeyResponse {
        public_key: derived_public_key,
        chain_code: derived_chain_code,
    }
}




/// Sends a transaction to the network that transfers the given amount to the
/// given destination, where the source of the funds is the canister itself
/// at the given derivation path.
pub async fn send(
    network: BitcoinNetwork,
    // path: ic_management_canister_types::DerivationPath,
    key_name: String,
    dst_address: String,
    amount: Satoshi,
    account: &Account
) -> (Vec<u8>, String) {
    // Get fee percentiles from previous transactions to estimate our own fee.
    let fee_percentiles = match  bitcoin_get_current_fee_percentiles(GetCurrentFeePercentilesRequest{network}).await {
        Ok(fee) => fee.0,
        Err(_) => vec![],
    };
    let own_public_key = read_public_key().await;
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
    // let own_address = account_to_p2wpkh_address(network, &own_public_key,&account).await;
    // print("Fetching UTXOs...");
    // Note that pagination may have to be used to get all UTXOs for the given address.
    // For the sake of simplicity, it is assumed here that the `utxo` field in the response
    // contains all UTXOs.
    let own_utxos = bitcoin_api::get_all_utxo_from_wallet();
    // ic_cdk::println!("own_utxo: {:?}", &own_utxos);
    let ecdsa_key = read_public_key().await;
    let derive_pubkey = derive_public_key(&ecdsa_key, &account).public_key;
    let compress_key = CompressedPublicKey::from_slice(&derive_pubkey).unwrap();
    let own_address = Address::p2wpkh(&compress_key, Network::Testnet);
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
        // path,
        amount,
        account,
    )
    .await;

    let signed_transaction_bytes = serialize(&signed_transaction);
    // eprintln!("{}", &format!(
    //     "Signed transaction: {}",
    //     hex::encode(&signed_transaction_bytes)
    // ));
    let res_vec = vec![0u8];
    match bitcoin_send_transaction(SendTransactionRequest{network, transaction: signed_transaction_bytes.clone() }).await {
    // match bitcoin_api::send_transaction(network, signed_transaction_bytes.clone()).await {
        Ok(()) => return (signed_transaction_bytes, signed_transaction.compute_txid().to_string()),
        Err(err) => return (res_vec,err.1)
    }

    
}

// Builds a transaction to send the given `amount` of satoshis to the
// destination address.
async fn build_transaction(
    own_public_key: &ECDSAPublicKey,
    own_address: &Address,
    own_utxos: &HashMap<JsonOutPoint, u64>,
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
            amount,       // mock derivation path
            account,
        )
        .await;

        let signed_tx_bytes_len = signed_transaction.total_size() as u64;

        if (signed_tx_bytes_len * fee_per_byte) / 1000 == total_fee {
            // print(&format!("Transaction built with fee {}.", total_fee));
            return transaction;
        } else {
            total_fee = (signed_tx_bytes_len * fee_per_byte) / 1000;
        }
    }
}



fn p2wpkh_script_code(pkhash: &[u8; 20]) -> bitcoin::ScriptBuf {
    use bitcoin::blockdata::opcodes;
    let push_bytes = PushBytesBuf::try_from(pkhash.to_vec()).unwrap();
    Builder::new()
        .push_opcode(opcodes::all::OP_DUP)
        .push_opcode(opcodes::all::OP_HASH160)
        .push_slice(push_bytes)
        .push_opcode(opcodes::all::OP_EQUALVERIFY)
        .push_opcode(opcodes::all::OP_CHECKSIG)
        .into_script()
}

fn build_transaction_with_fee(
    own_utxos: &HashMap<JsonOutPoint, u64>,
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
    for (outpoint, value) in own_utxos.iter() {
        total_spent += value;
        utxos_to_spend.push((outpoint, value));
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
                txid: Txid::from_raw_hash(Hash::from_slice(&utxo.txid()).unwrap()),
                vout: utxo.vout(),
            },
            sequence: bitcoin::Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::default(),
            script_sig: Script::builder().into_script(),
        })
        .collect();

    let mut outputs = vec![TxOut {
        script_pubkey: dst_address.script_pubkey(),
        value: Amount::from_sat(amount),
    }];

    let remaining_amount = total_spent - amount - fee;

    if remaining_amount >= DUST_THRESHOLD {
        outputs.push(TxOut {
            script_pubkey: own_address.script_pubkey(),
            value: Amount::from_sat(remaining_amount),
        });
    }

    Ok(Transaction {
        input: inputs,
        output: outputs,
        lock_time: LockTime::ZERO,
        version: Version(2),
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
    own_public_key: &ECDSAPublicKey,
    own_address: &Address,
    mut transaction: Transaction,
    key_name: String,
    // derivation_path: Vec<Vec<u8>>,
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
    
    let path = derivation_path(account).iter().map(|path| path.to_vec()).collect::<Vec<_>>();
    let pubkey = derive_public_key(own_public_key, account).public_key;
    // let pkhash = hash160(&pubkey);
    // let script_key = p2wpkh_script_code(&pkhash);
    // assert_eq!(
    //     own_address.script_pubkey(),
    //     script_key,
    //     "the script code of own address doesn't match the p2wpkh."
    // );
    // let path = ic_management_canister_types::DerivationPath::new(derivation_path(account));

    for (index, input) in transaction.input.iter_mut().enumerate() {

        let sighash = sighashcache.p2wpkh_signature_hash(index, &own_address.script_pubkey(), Amount::from_sat(amount), EcdsaSighashType::All).expect("build sighash failed");
        
        let signature =
        match get_sign_with_ecdsa(key_name.clone(), path.clone(), sighash.to_byte_array().to_vec())
            .await {
                Ok(sig) => sig,
                Err(_) => SignWithEcdsaResponse::default(),
        };

        // Convert signature to DER.
        let der_signature = sec1_to_der(signature.signature);
        let mut sig_with_hashtype = der_signature;
        sig_with_hashtype.push(SIG_HASH_TYPE.to_u32() as u8);
        // let sig_data = PushBytesBuf::try_from(sig_with_hashtype.as_slice().to_vec()).unwrap();
        // let pubkey_data = PushBytesBuf::try_from(pubkey.to_vec()).unwrap();
        let witness_sig = Signature::from_slice(&sig_with_hashtype).unwrap();
        let witness_pubkey = bitcoin::secp256k1::PublicKey::from_slice(&pubkey).unwrap();
        //  input.witness = Witness::p2wpkh(&witness_sig, &witness_pubkey);
        *sighashcache.witness_mut(index).unwrap() = Witness::p2wpkh(&witness_sig, &witness_pubkey);
        // input.witness.push(&sig_with_hashtype);
        // input.witness.push(&pubkey);
    }
    sighashcache.into_transaction()

    // transaction
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
    encode_bech32_new(&ripemd160(&sha256(public_key)), WitnessVersion::V0)
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
// fn encode_bech32(network: BitcoinNetwork, hash: &[u8], version: WitnessVersion) -> String {
//     use bech32::primitives::segwit::;

//     let hrp = hrp(network);
//     let witness_version: u5 =
//         u5::try_from_u8(version as u8).expect("bug: witness version must be smaller than 32");
//     let data: Vec<u5> = std::iter::once(witness_version)
//         .chain(
//             bech32::convert_bits(hash, 8, 5, true)
//                 .expect("bug: bech32 bit conversion failed on valid inputs")
//                 .into_iter()
//                 .map(|b| {
//                     u5::try_from_u8(b).expect("bug: bech32 bit conversion produced invalid outputs")
//                 }),
//         )
//         .collect();
//     match version {
//         WitnessVersion::V0 => bech32::encode(hrp, data, bech32::Variant::Bech32)
//             .expect("bug: bech32 encoding failed on valid inputs"),
//         WitnessVersion::V1 => bech32::encode(hrp, data, bech32::Variant::Bech32m)
//             .expect("bug: bech32m encoding failed on valid inputs"),
//     }
// }

fn encode_bech32_new(hash: &[u8], version: WitnessVersion) -> String 
{
    use bech32::Hrp;

    // use bech32::Bech32;
    // use bech32::encode;
    use bech32::segwit::encode_v0;
    let hrp = Hrp::parse_unchecked("tb");
    encode_v0(hrp, &hash).expect("failed to encode")
}



/// Converts a SEC1 ECDSA signature to the DER format.
///
/// # Panics
///
/// This function panics if:
/// * The input slice is not 64 bytes long.
/// * Either S or R signature components are zero.
pub fn sec1_to_der(sec1: Vec<u8>) -> Vec<u8> {
    // See:
    // * https://github.com/bitcoin/bitcoin/blob/5668ccec1d3785632caf4b74c1701019ecc88f41/src/script/interpreter.cpp#L97-L170
    // * https://github.com/bitcoin/bitcoin/blob/d08b63baa020651d3cc5597c85d5316cb39aaf59/src/secp256k1/src/ecdsa_impl.h#L183-L205
    // * https://security.stackexchange.com/questions/174095/convert-ecdsa-signature-from-plain-to-der-format
    // * "Mastering Bitcoin", 2nd edition, p. 140, "Serialization of signatures (DER)".

    fn push_integer(buf: &mut Vec<u8>, mut bytes: &[u8]) -> u8 {
        while !bytes.is_empty() && bytes[0] == 0 {
            bytes = &bytes[1..];
        }

        assert!(
            !bytes.is_empty(),
            "bug: one of the signature components is zero"
        );

        assert_ne!(bytes[0], 0);

        let neg = bytes[0] & 0x80 != 0;
        let n = if neg { bytes.len() + 1 } else { bytes.len() };
        debug_assert!(n <= u8::MAX as usize);

        buf.push(0x02);
        buf.push(n as u8);
        if neg {
            buf.push(0);
        }
        buf.extend_from_slice(bytes);
        n as u8
    }

    assert_eq!(
        sec1.len(),
        64,
        "bug: a SEC1 signature must be 64 bytes long"
    );

    let r = &sec1[..32];
    let s = &sec1[32..];

    let mut buf = Vec::with_capacity(72);
    // Start of the DER sequence.
    buf.push(0x30);
    // The length of the sequence:
    // Two bytes for integer markers and two bytes for lengths of the integers.
    buf.push(4);
    let rlen = push_integer(&mut buf, r);
    let slen = push_integer(&mut buf, s);
    buf[1] += rlen + slen; // Update the sequence length.
    buf
}