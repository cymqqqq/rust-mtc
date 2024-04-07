

use std::fmt;

use bitcoin::hashes::Hash;
use bitcoin::Network;
use bitcoin::{Txid, OutPoint};
use candid::{CandidType, Principal, Deserialize};
use ic_cdk::api::call::call_with_payment;
use ic_cdk::api::management_canister::bitcoin::{
    BitcoinAddress, BitcoinNetwork, GetBalanceRequest, GetCurrentFeePercentilesRequest,
    GetUtxosRequest, GetUtxosResponse, MillisatoshiPerByte, Satoshi, SendTransactionRequest,
};
use std::cell::RefCell;
use serde::Serialize;
// The fees for the various bitcoin endpoints.
const GET_BALANCE_COST_CYCLES: u64 = 100_000_000;
const GET_UTXOS_COST_CYCLES: u64 = 10_000_000_000;
const GET_CURRENT_FEE_PERCENTILES_CYCLES: u64 = 100_000_000;
const SEND_TRANSACTION_BASE_CYCLES: u64 = 5_000_000_000;
const SEND_TRANSACTION_PER_BYTE_CYCLES: u64 = 20_000_000;

thread_local! {
    static WALLET_STATE: RefCell<WalletState> = RefCell::new(WalletState::init());

}

#[derive(CandidType, Deserialize, Serialize, Debug, Clone)]
pub struct WalletState {
    pub unspend_utxo: Vec<(String, u64)>
}


impl WalletState {
    pub fn init() -> Self {
        Self { unspend_utxo: vec![] }
    }

    pub fn push_utxo(&mut self, outpoint: String, amount: u64) {
        self.unspend_utxo.push((outpoint, amount));
    }

    pub fn get_utxo(&self) -> Vec<(String, u64)> {
        self.unspend_utxo.clone()
    }

}

pub fn write_wallet_utxo(outpoint: String, amount: u64) {
    WALLET_STATE.with(|wallet_state| wallet_state.borrow_mut().push_utxo(outpoint, amount));
}

pub fn read_wallet_utxo() -> Vec<(String, u64)> {
    WALLET_STATE.with(|wallet_state| wallet_state.borrow().get_utxo())

}

pub async fn get_balance(network: BitcoinNetwork, address: String) -> u64 {
    let balance_res: Result<(Satoshi, ), _> = call_with_payment(
        Principal::management_canister(),
         "bitcoin_get_balance", 
         (GetBalanceRequest {
            address,
            network: network.into(),
            min_confirmations: None,
         },), GET_BALANCE_COST_CYCLES).await;
    balance_res.unwrap().0
}
#[derive(Serialize, Deserialize, Debug, CandidType, Clone)]
pub struct JsonOutPoint {
  txid: String,
  vout: u32,
}

impl JsonOutPoint {
    pub fn txid(&self) -> &[u8] {
        self.txid.as_bytes()
    }
    
    pub fn txid_hex(&self) -> &str {
        self.txid.as_str()
    }
    pub fn vout(&self) -> u32 {
        self.vout
    }
}
impl fmt::Display for JsonOutPoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.txid, self.vout)
    }
}
impl From<OutPoint> for JsonOutPoint {
  fn from(outpoint: OutPoint) -> Self {
    Self {
      txid: outpoint.txid.to_string(),
      vout: outpoint.vout,
    }
  }
}
pub async fn get_utxo(network: BitcoinNetwork, address: String) -> Vec<(String, u64)> {
    let utxo_res: Result<(GetUtxosResponse, ), _> = call_with_payment(
        Principal::management_canister(), 
        "bitcoin_get_utxos", 
        (GetUtxosRequest {
            address,
            network: network.into(),
            filter: None,
        }, ), GET_UTXOS_COST_CYCLES).await;
    let unspent_utxo = utxo_res.unwrap().0.utxos;
    // let mut unspent = Vec::new();
    unspent_utxo.into_iter()
        .map(|output| {
            let outpoint = OutPoint::new(Txid::from_slice(&output.outpoint.txid).expect("get txid failed"), output.outpoint.vout);
            let json_outpoint = JsonOutPoint::from(outpoint);
            let format_outpoint_str = format!("{}:{}", json_outpoint.txid_hex().to_string(), json_outpoint.vout());
            write_wallet_utxo(format_outpoint_str, output.value);
            // unspent.push((JsonOutPoint::from(outpoint), output.value));
        })
        .collect::<Vec<_>>();
    // unspent
    read_wallet_utxo()

}

pub async fn get_current_fee_percent(network: BitcoinNetwork) -> Vec<MillisatoshiPerByte> {
    let res: Result<(Vec<MillisatoshiPerByte>, ), _> = call_with_payment(
        Principal::management_canister(), 
        "bitcoin_get_current_fee_percentiles", 
        (GetCurrentFeePercentilesRequest {
            network: network.into(),
        }, ), GET_CURRENT_FEE_PERCENTILES_CYCLES).await;
    res.unwrap().0

}

pub async fn send_transaction(network: BitcoinNetwork, transaction: Vec<u8>) {
    let transaction_fee = SEND_TRANSACTION_BASE_CYCLES +
    (transaction.len() as u64) * SEND_TRANSACTION_PER_BYTE_CYCLES;
    let res: Result<(), _> = call_with_payment(
        Principal::management_canister(), 
        "bitcoin_send_transaction", 
        (SendTransactionRequest {
            network: network.into(),
            transaction
        }, ), transaction_fee).await;
    res.unwrap()
}
