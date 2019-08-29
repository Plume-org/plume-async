use super::request;
use base64;
use chrono::{naive::NaiveDateTime, DateTime, Duration, Utc};
use hex;
use openssl::{pkey::PKey, rsa::Rsa, sha::sha256};
use serde_json;

/// Returns (public key, private key)
pub fn gen_keypair() -> (Vec<u8>, Vec<u8>) {
    let keypair = Rsa::generate(2048).expect("sign::gen_keypair: key generation error");
    let keypair = PKey::from_rsa(keypair).expect("sign::gen_keypair: parsing error");
    (
        keypair
            .public_key_to_pem()
            .expect("sign::gen_keypair: public key encoding error"),
        keypair
            .private_key_to_pem_pkcs8()
            .expect("sign::gen_keypair: private key encoding error"),
    )
}

pub trait Signer {
    type Error;

    fn get_key_id(&self) -> String;

    /// Sign some data with the signer keypair
    fn sign(&self, to_sign: &str) -> Result<Vec<u8>, Self::Error>;
    /// Verify if the signature is valid
    fn verify(&self, data: &str, signature: &[u8]) -> Result<bool, Self::Error>;
}

pub trait Signable {
    fn sign<T>(&mut self, creator: &T) -> Result<&mut Self, ()>
    where
        T: Signer;
    fn verify<T>(self, creator: &T) -> bool
    where
        T: Signer;

    fn hash(data: &str) -> String {
        let bytes = data.as_bytes();
        hex::encode(sha256(bytes))
    }
}

impl Signable for serde_json::Value {
    fn sign<T: Signer>(&mut self, creator: &T) -> Result<&mut serde_json::Value, ()> {
        let creation_date = Utc::now().to_rfc3339();
        let mut options = json!({
            "type": "RsaSignature2017",
            "creator": creator.get_key_id(),
            "created": creation_date
        });

        let options_hash = Self::hash(
            &json!({
                "@context": "https://w3id.org/identity/v1",
                "created": creation_date
            })
            .to_string(),
        );
        let document_hash = Self::hash(&self.to_string());
        let to_be_signed = options_hash + &document_hash;

        let signature = base64::encode(&creator.sign(&to_be_signed).map_err(|_| ())?);

        options["signatureValue"] = serde_json::Value::String(signature);
        self["signature"] = options;
        Ok(self)
    }

    fn verify<T: Signer>(mut self, creator: &T) -> bool {
        let signature_obj =
            if let Some(sig) = self.as_object_mut().and_then(|o| o.remove("signature")) {
                sig
            } else {
                //signature not present
                return false;
            };
        let signature = if let Ok(sig) =
            base64::decode(&signature_obj["signatureValue"].as_str().unwrap_or(""))
        {
            sig
        } else {
            return false;
        };
        let creation_date = &signature_obj["created"];
        let options_hash = Self::hash(
            &json!({
                "@context": "https://w3id.org/identity/v1",
                "created": creation_date
            })
            .to_string(),
        );
        let creation_date = creation_date.as_str();
        if creation_date.is_none() {
            return false;
        }
        let creation_date = DateTime::parse_from_rfc3339(creation_date.unwrap());
        if creation_date.is_err() {
            return false;
        }
        let diff = creation_date.unwrap().signed_duration_since(Utc::now());
        let future = Duration::hours(12);
        let past = Duration::hours(-12);
        if !(diff < future && diff > past) {
            return false;
        }
        let document_hash = Self::hash(&self.to_string());
        let to_be_signed = options_hash + &document_hash;
        creator.verify(&to_be_signed, &signature).unwrap_or(false)
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SignatureValidity {
    Invalid,
    ValidNoDigest,
    Valid,
    Absent,
    Outdated,
}

impl SignatureValidity {
    pub fn is_secure(self) -> bool {
        self == SignatureValidity::Valid
    }
}
