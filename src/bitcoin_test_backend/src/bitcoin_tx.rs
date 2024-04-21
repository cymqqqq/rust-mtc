use std::{collections::HashMap, str::FromStr};

use bitcoin::{
    absolute::LockTime, 
    consensus::serialize, 
    hashes::Hash, 
    key::{
        constants::SCHNORR_SIGNATURE_SIZE, Secp256k1}, 
        opcodes, 
        script::Builder, 
        secp256k1::schnorr, 
        sighash::{self, SighashCache}, 
        taproot::{ControlBlock, LeafVersion, Signature, TaprootBuilder}, 
        transaction::Version, Address, Amount, FeeRate, KnownHrp, Network, OutPoint, PublicKey, Script, Sequence, TapLeafHash, Transaction, TxIn, TxOut, Txid, Witness, XOnlyPublicKey};
use hex::ToHex;
use ic_cdk::api::management_canister::bitcoin::{bitcoin_send_transaction, BitcoinNetwork, SendTransactionRequest};
use icrc_ledger_types::icrc1::account::Account;
// use bitcoin::secp256k1::schnorr::Signature;
use crate::{
    wallet::state::{get_all_utxo_from_wallet, JsonOutPoint}, 
    wallet::send_btc::sign_transaction_p2pkh, 
    utils::read_public_key, 
    inscription::Inscription, 
    schnnor::{schnorr_public_key, sign_with_schnorr}, 
};
use crate::wallet::address::account_to_p2pkh_address;

fn transform_network(network: BitcoinNetwork) -> Network {
    match network {
        BitcoinNetwork::Mainnet => Network::Bitcoin,
        BitcoinNetwork::Testnet => Network::Testnet,
        BitcoinNetwork::Regtest => Network::Regtest,
    }
}

pub async fn inscribe(
    key_name: String,
    network: BitcoinNetwork,
    content_type: Option<Vec<u8>>,
    body: Option<Vec<u8>>,
    dst_address: &Address,
    fee_rate: u64,
    account: Account,
) -> (String, String) {
    let inscription = Inscription::new(content_type, body);
    let derivation_path = vec![];
    let sender_public_key = read_public_key().await;
    let sender_address_string = account_to_p2pkh_address(network, &sender_public_key, &account).await;
    let sender_utxo = get_all_utxo_from_wallet();
    let sender_address = Address::from_str(&sender_address_string).unwrap().require_network(transform_network(network)).unwrap();
    let raw_schnnor_public_key = schnorr_public_key(&key_name, derivation_path.clone()).await;
    let schnnor_public_key = PublicKey::from_slice(&raw_schnnor_public_key).unwrap();
    let fee_rate = FeeRate::from_sat_per_vb(fee_rate).unwrap();
    let (commit_tx, reveal_tx) = build_inscription_transaction(
        &key_name, 
        network, 
        &sender_utxo, 
        &sender_public_key.public_key, 
        &sender_address,
        dst_address, 
        schnnor_public_key.into(), 
        inscription, 
        derivation_path, 
        fee_rate).await.expect("build inscription transaction failed");
    let commit_tx_bytes = serialize(&commit_tx);
    bitcoin_send_transaction(SendTransactionRequest{network, transaction: commit_tx_bytes}).await;
    let reveal_tx_bytes  = serialize(&reveal_tx);
    bitcoin_send_transaction(SendTransactionRequest { network, transaction: reveal_tx_bytes}).await;
    (commit_tx.compute_txid().encode_hex(), reveal_tx.compute_txid().encode_hex())

}


async fn build_inscription_transaction(
    key_name: &str,
    network: BitcoinNetwork,
    utxos: &HashMap<JsonOutPoint, u64>,
    sender_public_key: &[u8],
    sender_address: &Address,
    dst_address: &Address,
    schnnor_public_key: XOnlyPublicKey,
    inscription: Inscription,
    derivation_path: Vec<Vec<u8>>,
    fee_rate: FeeRate,
) -> Result<(Transaction, Transaction), String> {
    let mut builder = Builder::new();

    // builder = inscription.append_reveal_script_to_builder(builder);
    
    let secp256 = Secp256k1::new();
    builder = builder
            .push_slice(&schnnor_public_key.serialize())
            .push_opcode(opcodes::all::OP_CHECKSIG);
    let reveal_script = builder.into_script();
    let taproot_spend_info = TaprootBuilder::new()
            .add_leaf(0, reveal_script.clone())
            .expect("add leaf failed")
            .finalize(&secp256, schnnor_public_key)
            .expect("finalize taproot builder failed");
    
    let control_block = taproot_spend_info 
            .control_block(&(reveal_script.clone(), LeafVersion::TapScript))
            .expect("compute control block failed");

    let commit_tx_address = Address::p2tr_tweaked(taproot_spend_info.output_key(), KnownHrp::Testnets);

    let mut reveal_inputs = vec![];
    let mut reveal_outputs = vec![TxOut {
        script_pubkey: dst_address.script_pubkey(),
        value: Amount::from_sat(0),
    }];

    let commit_input_index = 0;
    let (_, reveal_fee) = build_reveal_transaction(
        &control_block, 
        fee_rate, 
        reveal_inputs.clone(), 
        commit_input_index, 
        reveal_outputs, 
        &reveal_script);
    let mut utxo_to_spent = vec![];
    let mut total_spent = 0;
    utxos.iter().map(|utxo| {total_spent += utxo.1; utxo_to_spent.push(utxo.0);} );
    
    let total_sats_amount = Amount::from_sat(total_spent);

    let inputs = utxo_to_spent
        .into_iter()
        .map(|utxo| TxIn {
            previous_output: OutPoint {
                txid: Txid::from_raw_hash(Hash::from_slice(&utxo.txid()).unwrap()),
                vout: utxo.vout(),
            },
            sequence: Sequence::ZERO,
            witness: Witness::new(),
            script_sig: Script::new().into(),
        })
        .collect();

    let mut unsigned_commit_tx = Transaction {
        input: inputs,
        output: vec![TxOut {
            script_pubkey: commit_tx_address.script_pubkey(),
            value: total_sats_amount,
        }],
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    // set p2pkh sig vbytes to 73
    let sig_vbytes = 73;
    let commit_fee = fee_rate.fee_vb(unsigned_commit_tx.vsize() as u64 + sig_vbytes).unwrap();
    unsigned_commit_tx.output[0].value = total_sats_amount - commit_fee;
    let commit_tx = sign_transaction_p2pkh(
        sender_public_key, 
        sender_address, 
        unsigned_commit_tx, 
        &key_name, 
        derivation_path.clone()).await;
    let (vout, _commit_output) = commit_tx
            .output
            .iter()
            .enumerate()
            .find(|(_vout, output)| output.script_pubkey == commit_tx_address.script_pubkey())
            .expect("should find sat amount/inscription output");
    reveal_inputs[commit_input_index] = OutPoint {
        txid: commit_tx.compute_txid(),
        vout: vout.try_into().unwrap(),
    };

    reveal_outputs = vec![TxOut {
        script_pubkey: dst_address.script_pubkey(),
        value: total_sats_amount - commit_fee - reveal_fee,
    }];

    let mut reveal_tx = Transaction {
        input: reveal_inputs
            .iter()
            .map(|outpoint| TxIn {
                previous_output: *outpoint,
                script_sig: Builder::new().into_script(),
                witness: Witness::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            })
            .collect(),
        output: reveal_outputs,
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    let mut sighasher = SighashCache::new(&mut reveal_tx);
    let sighash = sighasher
        .taproot_script_spend_signature_hash(
            commit_input_index, 
            &sighash::Prevouts::All(commit_tx.output.as_slice()), 
            TapLeafHash::from_script(&reveal_script, LeafVersion::TapScript), 
            bitcoin::TapSighashType::Default)
            .expect("failed to contract sighash");

    let sig = sign_with_schnorr(&key_name, derivation_path.clone(), sighash.to_byte_array().to_vec()).await;
    let witness = sighasher
            .witness_mut(commit_input_index)
            .expect("failed to get mutable witness");
    witness.push(
        Signature {
            signature: schnorr::Signature::from_slice(sig.as_slice()).expect("parse sig failed"),
            sighash_type: bitcoin::TapSighashType::Default,
        }
        .to_vec(),
    );

    witness.push(reveal_script);
    witness.push(&control_block.serialize());
    Ok((commit_tx, reveal_tx))

    

}




fn build_reveal_transaction(
    control_block: &ControlBlock,
    fee_rate: FeeRate,
    inputs: Vec<OutPoint>,
    commit_input_index: usize,
    outputs: Vec<TxOut>,
    script: &Script
) -> (Transaction, Amount) {
    let reveal_transaction = Transaction {
        input: inputs
            .iter()
            .map(|outpoint| TxIn {
                previous_output: *outpoint,
                script_sig: Builder::new().into_script(),
                witness: Witness::new(),
                sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            })
            .collect(),
        output: outputs,
        lock_time: LockTime::ZERO,
        version: Version(2),
    };

    let fee = {
        let mut reveal_tx = reveal_transaction.clone();
        for (idx, txin) in reveal_tx.input.iter_mut().enumerate() {
            if idx == commit_input_index {
                txin.witness.push(Signature::from_slice(&[0; SCHNORR_SIGNATURE_SIZE]).unwrap().to_vec());
                txin.witness.push(script);
                txin.witness.push(&control_block.serialize());
            } else {
                txin.witness = Witness::from_slice(&[&[0; SCHNORR_SIGNATURE_SIZE]]);
            }
        }
        fee_rate.fee_vb(reveal_tx.vsize().try_into().unwrap()).unwrap()
    };
    (reveal_transaction, fee)
}