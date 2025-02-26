use hyperlane_core::ChainCommunicationError;

/// Errors from the crates specific to the hyperlane-tron implementation.
/// This error can then be converted into the broader error type
/// in hyperlane-core using the `From` trait impl
#[derive(Debug, thiserror::Error)]
pub enum HyperlaneTronError {
    /// CoreError error
    #[error("{0}")]
    CoreError(heliosphere_core::Error),
    /// SignatureError error
    #[error("{0}")]
    SignatureError(#[from] heliosphere_signer::keypair::KeypairSignError),
    /// ClientError error
    #[error("{0}")]
    ClientError(#[from] heliosphere::Error),
    /// ProviderError error
    #[error("{0}")]
    ProviderError(#[from] ethers::providers::ProviderError),
    /// ABI error
    #[error("{0}")]
    AbiError(#[from] ethers::core::abi::AbiError),
}

// Can't use macro because `heliosphere_core::Error` doesn't implement `Error` trait
impl From<heliosphere_core::Error> for HyperlaneTronError {
    fn from(err: heliosphere_core::Error) -> Self {
        HyperlaneTronError::CoreError(err)
    }
}

impl From<HyperlaneTronError> for ChainCommunicationError {
    fn from(value: HyperlaneTronError) -> Self {
        ChainCommunicationError::from_other(value)
    }
}
