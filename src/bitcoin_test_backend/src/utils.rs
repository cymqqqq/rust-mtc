use icrc_ledger_types::icrc1::account::Account;
use serde_bytes::ByteBuf;
use sha2::{Digest, Sha256};
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

pub fn sha256(data: &[u8]) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

pub fn ripemd160(data: &[u8]) -> Vec<u8> {
    let mut hasher = ripemd::Ripemd160::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// SHA-256 followed by Ripemd160, also known as HASH160.
pub fn hash160(bytes: &[u8]) -> [u8; 20] {
    use ripemd::Ripemd160;
    Ripemd160::digest(Sha256::digest(bytes)).into()
}
