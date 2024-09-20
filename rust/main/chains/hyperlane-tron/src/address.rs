use ethers::types::H160;
use heliosphere::core::Address;

use hyperlane_core::H256;

use crate::HyperlaneTronError;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TronAddress(Address);

impl From<H160> for TronAddress {
    fn from(value: H160) -> Self {
        TronAddress(Address::from(value))
    }
}

impl From<TronAddress> for H160 {
    fn from(value: TronAddress) -> Self {
        H160::from(value.0)
    }
}

impl TryFrom<H256> for TronAddress {
    type Error = HyperlaneTronError;

    fn try_from(value: H256) -> Result<Self, Self::Error> {
        let mut bytes = [0u8; 21];
        bytes.copy_from_slice(&value[11..]);

        let address = Address::new(bytes)?;
        Ok(TronAddress(address))
    }
}

impl From<TronAddress> for H256 {
    fn from(value: TronAddress) -> Self {
        let mut bytes = [0u8; 32];
        bytes[11..].copy_from_slice(value.0.as_bytes());

        H256::from(bytes)
    }
}

impl AsRef<Address> for TronAddress {
    fn as_ref(&self) -> &Address {
        &self.0
    }
}
