use async_trait::async_trait;
use std::sync::Arc;
use tracing::instrument;

use hyperlane_core::{
    Announcement, ChainCommunicationError, ChainResult, ContractLocator, HyperlaneChain,
    HyperlaneContract, HyperlaneDomain, HyperlaneProvider, SignedType, TxOutcome,
    ValidatorAnnounce, H160, H256, U256,
};

use crate::interfaces::i_validator_announce::{
    AnnounceCall, IValidatorAnnounce as ValidatorAnnounceContract,
};
use crate::{ConnectionConf, Signer, TronAddress, TronEthClient, TronProvider};

use super::utils::send_transaction;

/// A reference to a ValidatorAnnounce contract on some Tron chain
#[derive(Debug)]
pub struct TronValidatorAnnounce {
    contract: Arc<ValidatorAnnounceContract<TronEthClient>>,
    provider: TronProvider,
    signer: Option<Signer>,
}

impl TronValidatorAnnounce {
    pub fn new(
        conf: ConnectionConf,
        locator: ContractLocator,
        signer: Option<Signer>,
    ) -> ChainResult<Self> {
        let address = TronAddress::try_from(locator.address)?;
        let provider = TronProvider::new(locator.domain.clone(), conf)?;
        let contract = Arc::new(ValidatorAnnounceContract::new(
            address,
            provider.eth_client.clone(),
        ));

        Ok(TronValidatorAnnounce {
            contract,
            provider,
            signer,
        })
    }
}

impl HyperlaneContract for TronValidatorAnnounce {
    fn address(&self) -> H256 {
        TronAddress::from(self.contract.address()).into()
    }
}

impl HyperlaneChain for TronValidatorAnnounce {
    fn domain(&self) -> &HyperlaneDomain {
        self.provider.domain()
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        self.provider.provider()
    }
}

#[async_trait]
impl ValidatorAnnounce for TronValidatorAnnounce {
    async fn get_announced_storage_locations(
        &self,
        validators: &[H256],
    ) -> ChainResult<Vec<Vec<String>>> {
        let validators = validators.iter().map(|v| H160::from(*v).into()).collect();
        let locations = self
            .contract
            .get_announced_storage_locations(validators)
            .call()
            .await?;

        Ok(locations)
    }

    #[instrument(err, ret, skip(self))]
    async fn announce(&self, announcement: SignedType<Announcement>) -> ChainResult<TxOutcome> {
        let signer = self
            .signer
            .as_ref()
            .ok_or(ChainCommunicationError::SignerUnavailable)?;

        let serialized_signature: [u8; 65] = announcement.signature.into();

        send_transaction(
            &self.provider,
            &self.contract.address().into(),
            AnnounceCall {
                validator: announcement.value.validator.into(),
                storage_location: announcement.value.storage_location,
                signature: serialized_signature.into(),
            },
            signer,
            None,
        )
        .await
        .map_err(Into::into)
    }

    #[instrument(ret, skip(self))]
    async fn announce_tokens_needed(
        &self,
        _announcement: SignedType<Announcement>,
    ) -> Option<U256> {
        // TODO: implement it
        None
    }
}
