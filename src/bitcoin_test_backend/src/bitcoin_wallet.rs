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
}, ecdsa::{sign_with_ecdsa, SignWithEcdsaResponse}};
use icrc_ledger_types::icrc1::account::Account;

use crate::utils::*;
// use crate::ecdsa_api::{DerivationPath};
const SIG_HASH_TYPE: EcdsaSighashType = EcdsaSighashType::All;










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

// Sign a p2wpkh bitcoin transaction.

async fn sign_transaction
(
    own_public_key: &ECDSAPublicKey,
    own_address: &Address,
    mut transaction: Transaction,
    key_name: String,
    amount: Satoshi,
    account: &Account,
) -> Transaction
{
    // Verify that our own address is P2wPKH.
    assert_eq!(
        own_address.address_type(),
        Some(AddressType::P2wpkh),
        "This example supports signing p2wpkh addresses only."
    );
    let mut sighashcache = SighashCache::new(transaction.clone());
    
    let path = derivation_path(account).iter().map(|path| path.to_vec()).collect::<Vec<_>>();
    let pubkey = derive_public_key(own_public_key, account).public_key;

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
         input.witness = Witness::p2wpkh(&witness_sig, &witness_pubkey);
        // *sighashcache.witness_mut(index).unwrap() = Witness::p2wpkh(&witness_sig, &witness_pubkey);
        input.witness.push(&sig_with_hashtype);
        input.witness.push(&pubkey);
    }
    // sighashcache.into_transaction()

    transaction
}





pub async fn sign_transaction_p2pkh(
    own_public_key: &[u8],
    own_address: &Address,
    mut transaction: Transaction,
    key_name: &str,
    derivation_path: Vec<Vec<u8>>,
) -> Transaction
{
    // Verify that our own address is P2PKH.
    assert_eq!(
        own_address.address_type(),
        Some(AddressType::P2pkh),
        "Not a correct p2pkh address."
    );

    let txclone = transaction.clone();
    for (index, input) in transaction.input.iter_mut().enumerate() {
        let sighash = SighashCache::new(&txclone)
            .legacy_signature_hash(index, &own_address.script_pubkey(), SIG_HASH_TYPE.to_u32())
            .unwrap();

        let signature = match get_sign_with_ecdsa(
            key_name.to_string(),
            derivation_path.clone(),
            sighash.as_byte_array().to_vec(),
        )
        .await {
            Ok(sig) => sig,
            Err(_) => SignWithEcdsaResponse::default(),
        };

        // Convert signature to DER.
        let der_signature = sec1_to_der(signature.signature);

        let mut sig_with_hashtype = der_signature;
        sig_with_hashtype.push(SIG_HASH_TYPE.to_u32() as u8);

        let sig_with_hashtype_push_bytes = PushBytesBuf::try_from(sig_with_hashtype).unwrap();
        let own_public_key_push_bytes = PushBytesBuf::try_from(own_public_key.to_vec()).unwrap();
        input.script_sig = Builder::new()
            .push_slice(sig_with_hashtype_push_bytes)
            .push_slice(own_public_key_push_bytes)
            .into_script();
        input.witness.clear();
    }

    transaction
}
