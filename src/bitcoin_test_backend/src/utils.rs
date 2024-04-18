use icrc_ledger_types::icrc1::account::Account;
use serde_bytes::ByteBuf;
use sha2::{Digest, Sha256};
use ic_crypto_extended_bip32::{DerivationPath, DerivationIndex, ExtendedBip32DerivationOutput};
use ic_management_canister_types::ECDSAPublicKeyResponse;
use crate::types::ECDSAPublicKey;


#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WitnessVersion {
    V0 = 0,
    V1 = 1,
}
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