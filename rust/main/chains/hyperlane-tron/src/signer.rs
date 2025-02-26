use std::{
    fmt::{Debug, Formatter},
    sync::Arc,
};

use heliosphere_signer::{k256::ecdsa::SigningKey, keypair::Keypair, signer::Signer as _};

use crate::HyperlaneTronError;

#[derive(Clone)]
/// Signer for tron chain
pub struct Signer(pub(crate) Arc<Keypair>);

impl Signer {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HyperlaneTronError> {
        let signing_key = SigningKey::from_slice(&bytes)?;
        let key_pair = Keypair::from_signing_key(signing_key);

        Ok(Signer(Arc::new(key_pair)))
    }

    pub fn address(&self) -> String {
        self.0.address().as_base58()
    }
}

impl Debug for Signer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Signer { ... }")
    }
}
