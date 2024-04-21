

// use std::fmt;
// use std::io::{Read, Write};

use bitcoin::hashes::Hash;
// use bitcoin::Network;
use bitcoin::{Txid, OutPoint};
use candid::{CandidType, Principal, Deserialize};
use ic_cdk::api::call::call_with_payment;
use ic_cdk::api::management_canister::bitcoin::{
 BitcoinNetwork,
    GetUtxosRequest, GetUtxosResponse,
};
use std::cell::RefCell;
use serde::Serialize;
use std::collections::HashMap;
// The fees for the various bitcoin endpoints.
const GET_UTXOS_COST_CYCLES: u64 = 10_000_000_000;

thread_local! {
    static WALLET_STATE: RefCell<WalletState> = RefCell::new(WalletState::init());

}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone)]
pub struct WalletState {
    pub unspend_utxo: HashMap<JsonOutPoint, u64>
}


impl WalletState {
    pub fn init() -> Self {
        Self { unspend_utxo: HashMap::new() }
    }

    pub fn push_utxo(&mut self, outpoint: &JsonOutPoint, amount: u64) {
        self.unspend_utxo.insert(outpoint.to_owned(), amount);
    
    }

    pub fn get_utxo(&self) -> HashMap<JsonOutPoint, u64> {
        self.unspend_utxo.clone()
    }

}

pub fn write_wallet_utxo(outpoint: JsonOutPoint, amount: u64) {
    WALLET_STATE.with(|wallet_state| wallet_state.borrow_mut().push_utxo(&outpoint, amount));
}

pub fn get_all_utxo_from_wallet() -> HashMap<JsonOutPoint, u64> {
    WALLET_STATE.with(|wallet_state| wallet_state.borrow().get_utxo())

}

pub fn read_wallet_utxo() -> Vec<(String, u64)> {
    let mut utxo_set = Vec::new();
    WALLET_STATE.with(|wallet_state| {wallet_state
        .borrow()
        .get_utxo()
        .iter()
        .for_each(|(outpoint, amount)| {
            let outpoint_str = Txid::from_raw_hash(Hash::from_slice(outpoint.txid()).unwrap()).to_string();
            let vout = outpoint.vout();
            let utxo_str = format!("{:?}:{}", outpoint_str, vout);
            utxo_set.push((utxo_str, *amount));
        });
    });
    utxo_set
}

#[derive(Serialize, Deserialize, Debug, CandidType, Clone, PartialEq, PartialOrd, Eq, Hash)]
pub struct JsonOutPoint {
  txid: Vec<u8>,
  vout: u32,
}

impl JsonOutPoint {
    pub fn txid(&self) -> &[u8] {
        self.txid.as_slice()
    }
    
    pub fn vout(&self) -> u32 {
        self.vout
    }
}

impl From<OutPoint> for JsonOutPoint {
  fn from(outpoint: OutPoint) -> Self {
    Self {
      txid: outpoint.txid.to_byte_array().to_vec(),
      vout: outpoint.vout,
    }
  }
}
// tb1qnh2pq8ltrnk5qcqssu5wxhqwgg53s48fw7glv2

pub async fn update_utxo(network: BitcoinNetwork, address: String) -> Vec<(String, u64)> {
    let utxo_res: Result<(GetUtxosResponse, ), _> = call_with_payment(
        Principal::management_canister(), 
        "bitcoin_get_utxos", 
        (GetUtxosRequest {
            address,
            network: network.into(),
            filter: None,
        }, ), GET_UTXOS_COST_CYCLES).await;
    let unspent_utxo = utxo_res.unwrap().0.utxos;
    unspent_utxo.into_iter()
        .for_each(|output| {
            let outpoint = OutPoint::new(Txid::from_slice(&output.outpoint.txid).expect("get txid failed"), output.outpoint.vout);
            let json_outpoint = JsonOutPoint::from(outpoint);
            write_wallet_utxo(json_outpoint, output.value);
        });
    // unspent
    read_wallet_utxo()

}

