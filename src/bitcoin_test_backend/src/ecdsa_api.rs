use crate::types::*;
use candid::Principal;
use std::cell::RefCell;
use ic_cdk::{api::{call::{call_with_payment, RejectionCode}, management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId, EcdsaPublicKeyResponse, SignWithEcdsaArgument, SignWithEcdsaResponse}}, call};
use ic_cdk::api::management_canister::ecdsa::{ecdsa_public_key, sign_with_ecdsa};
use ic_cdk::api::management_canister::ecdsa::{EcdsaPublicKeyArgument};
use ic_management_canister_types::{DerivationPath, ECDSAPublicKeyArgs, ECDSAPublicKeyResponse, SignWithECDSAArgs, SignWithECDSAReply};
// The fee for the `sign_with_ecdsa` endpoint using the test key.
const SIGN_WITH_ECDSA_COST_CYCLES: u64 = 10_000_000_000;
/// Represents an error from a management canister call, such as
/// `sign_with_ecdsa` or `bitcoin_send_transaction`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallError {
    method: String,
    reason: Reason,
}
thread_local! {
    static KEY: RefCell<Option<ECDSAPublicKey>> = RefCell::default();

  
}

pub async fn write_public_key(public_key: &ECDSAPublicKey) {
    KEY.with(|key_state| *key_state.borrow_mut() = Some(public_key.clone()));
}

pub async fn read_public_key() -> ECDSAPublicKey {
    KEY.with(|key_state| {let key = key_state.borrow().clone();
        key.unwrap()
    }
)
}

/// Fetches the ECDSA public key of the canister.
pub async fn get_ecdsa_public_key(
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
) -> Result<EcdsaPublicKeyResponse, String> {
    // Retrieve the public key of this canister at the given derivation path
    // from the ECDSA API.
    let arg = EcdsaPublicKeyArgument {
        canister_id: Some(Principal::management_canister()),
        derivation_path,
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: key_name,
        },
    };
    match ecdsa_public_key(arg).await {
        Ok(ecdsa_key) => Ok(ecdsa_key.0),
        Err(err) => Err(err.1)
    }
}

pub async fn get_sign_with_ecdsa(
    key_name: String,
    derivation_path: Vec<Vec<u8>>,
    message_hash: Vec<u8>
) -> Result<SignWithEcdsaResponse, String> {
    // Retrieve the public key of this canister at the given derivation path
    // from the ECDSA API.
    let arg = SignWithEcdsaArgument {
        message_hash,
        derivation_path,
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: key_name,
        },
    };
    match sign_with_ecdsa(arg).await {
        Ok(ecdsa_key) => Ok(ecdsa_key.0),
        Err(err) => Err(err.1)
    }
}
// pub async fn sign_with_ecdsa(
//     key_name: String,
//     derivation_path: &DerivationPath,
//     message_hash: [u8; 32]
// ) -> Vec<u8> {
//     let res: Result<(SignWithECDSAReply, ), _> = call_with_payment(
//         Principal::management_canister(), 
//         "sign_with_ecdsa", 
//         (SignWithECDSAArgs {
//             message_hash,
//             derivation_path: derivation_path.clone(),
//             key_id: EcdsaKeyId {
//                 curve: EcdsaCurve::Secp256k1,
//                 name: key_name.clone(),
//             },
//         },), SIGN_WITH_ECDSA_COST_CYCLES).await;
//     res.unwrap().0.signature
// }

/// Initializes the Minter ECDSA public key. This function must be called
/// before any endpoint runs its logic.
pub async fn init_ecdsa_public_key() -> ECDSAPublicKey {
    // if let Some(key) = read_state(|s| s.ecdsa_public_key.clone()) {
    //     return key;
    // };
    // let key_name = read_state(|s| s.ecdsa_key_name.clone());
    let key_name =  "test_key_1".to_string();
    // log!(P1, "Fetching the ECDSA public key {}", &key_name);
    let ecdsa_public_key =
        match get_ecdsa_public_key(key_name, vec![vec![]]).await {
            Ok(key) => key,
            Err(_) => EcdsaPublicKeyResponse::default(),
        };
    let key = ECDSAPublicKey {
        public_key: ecdsa_public_key.public_key,
        chain_code: ecdsa_public_key.chain_code,
    };
    write_public_key(&key).await;
            // .unwrap_or_else(|e| ic_cdk::trap(&format!("failed to retrieve ECDSA public key: {e}")));
    // log!(
    //     P1,
    //     "ECDSA public key set to {}, chain code to {}",
    //     hex::encode(&ecdsa_public_key.public_key),
    //     hex::encode(&ecdsa_public_key.chain_code)
    // );
    // mutate_state(|s| {
    //     s.ecdsa_public_key = Some(ecdsa_public_key.clone());
    // });
    key
}