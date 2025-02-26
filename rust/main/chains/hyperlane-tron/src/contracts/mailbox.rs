use std::fmt::Debug;
use std::ops::RangeInclusive;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::instrument;

use hyperlane_core::{
    rpc_clients::call_and_retry_indefinitely, utils::bytes_to_hex, ChainCommunicationError,
    ChainResult, ContractLocator, HyperlaneChain, HyperlaneContract, HyperlaneDomain,
    HyperlaneMessage, HyperlaneProvider, Indexed, Indexer, LogMeta, Mailbox, RawHyperlaneMessage,
    ReorgPeriod, SequenceAwareIndexer, TxCostEstimate, TxOutcome, H256, H512, U256,
};

use crate::interfaces::i_mailbox::{DispatchFilter, IMailbox as MailboxContract, ProcessCall};
use crate::{ConnectionConf, Signer, TronAddress, TronEthClient, TronProvider};

use super::utils::{
    call_with_reorg_period, fetch_raw_logs_and_meta, get_finalized_block_number, send_transaction,
};

/// Struct that retrieves event data for a Tron mailbox
#[derive(Debug, Clone)]
pub struct TronMailboxIndexer {
    contract: Arc<MailboxContract<TronEthClient>>,
    provider: TronProvider,
    reorg_period: ReorgPeriod,
}

impl TronMailboxIndexer {
    pub fn new(
        conf: ConnectionConf,
        locator: ContractLocator,
        reorg_period: ReorgPeriod,
    ) -> ChainResult<Self> {
        let address = TronAddress::try_from(locator.address)?;
        let provider = TronProvider::new(locator.domain.clone(), conf)?;
        let contract = Arc::new(MailboxContract::new(address, provider.eth_client.clone()));

        Ok(TronMailboxIndexer {
            contract,
            provider,
            reorg_period,
        })
    }
}

#[async_trait]
impl Indexer<HyperlaneMessage> for TronMailboxIndexer {
    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        get_finalized_block_number(&self.provider, &self.reorg_period).await
    }

    /// Note: This call may return duplicates depending on the provider used
    #[instrument(err, skip(self))]
    #[allow(clippy::blocks_in_conditions)] // TODO: `rustc` 1.80.1 clippy issue
    async fn fetch_logs_in_range(
        &self,
        range: RangeInclusive<u32>,
    ) -> ChainResult<Vec<(Indexed<HyperlaneMessage>, LogMeta)>> {
        let mut events: Vec<(Indexed<HyperlaneMessage>, LogMeta)> = self
            .contract
            .dispatch_filter()
            .from_block(*range.start())
            .to_block(*range.end())
            .query_with_meta()
            .await?
            .into_iter()
            .map(|(event, meta)| {
                (
                    HyperlaneMessage::from(event.message.to_vec()).into(),
                    meta.into(),
                )
            })
            .collect();

        events.sort_by(|a, b| a.0.inner().nonce.cmp(&b.0.inner().nonce));
        Ok(events)
    }

    async fn fetch_logs_by_tx_hash(
        &self,
        tx_hash: H512,
    ) -> ChainResult<Vec<(Indexed<HyperlaneMessage>, LogMeta)>> {
        let raw_logs_and_meta = call_and_retry_indefinitely(|| {
            let provider = self.provider.clone();
            let contract = self.contract.address();
            Box::pin(async move {
                fetch_raw_logs_and_meta::<DispatchFilter>(&provider, contract, tx_hash).await
            })
        })
        .await;
        let logs = raw_logs_and_meta
            .into_iter()
            .map(|(log, log_meta)| {
                (
                    HyperlaneMessage::from(log.message.to_vec()).into(),
                    log_meta,
                )
            })
            .collect();
        Ok(logs)
    }
}

#[async_trait]
impl SequenceAwareIndexer<HyperlaneMessage> for TronMailboxIndexer {
    #[instrument(err, skip(self), ret)]
    #[allow(clippy::blocks_in_conditions)] // TODO: `rustc` 1.80.1 clippy issue
    async fn latest_sequence_count_and_tip(&self) -> ChainResult<(Option<u32>, u32)> {
        let tip = Indexer::<HyperlaneMessage>::get_finalized_block_number(self).await?;
        let sequence = self.contract.nonce().block(u64::from(tip)).call().await?;
        Ok((Some(sequence), tip))
    }
}

#[async_trait]
impl Indexer<H256> for TronMailboxIndexer {
    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        get_finalized_block_number(&self.provider, &self.reorg_period).await
    }

    /// Note: This call may return duplicates depending on the provider used
    #[instrument(err, skip(self))]
    #[allow(clippy::blocks_in_conditions)] // TODO: `rustc` 1.80.1 clippy issue
    async fn fetch_logs_in_range(
        &self,
        range: RangeInclusive<u32>,
    ) -> ChainResult<Vec<(Indexed<H256>, LogMeta)>> {
        Ok(self
            .contract
            .process_id_filter()
            .from_block(*range.start())
            .to_block(*range.end())
            .query_with_meta()
            .await?
            .into_iter()
            .map(|(event, meta)| (Indexed::new(H256::from(event.message_id)), meta.into()))
            .collect())
    }
}

#[async_trait]
impl SequenceAwareIndexer<H256> for TronMailboxIndexer {
    async fn latest_sequence_count_and_tip(&self) -> ChainResult<(Option<u32>, u32)> {
        // A blanket implementation for this trait is fine for the EVM.
        // TODO: Consider removing `Indexer` as a supertrait of `SequenceAwareIndexer`
        let tip = Indexer::<H256>::get_finalized_block_number(self).await?;
        Ok((None, tip))
    }
}

/// A reference to a Mailbox contract on some Tron chain
#[derive(Debug)]
pub struct TronMailbox {
    contract: Arc<MailboxContract<TronEthClient>>,
    provider: TronProvider,
    signer: Option<Signer>,
}

impl TronMailbox {
    pub fn new(
        conf: ConnectionConf,
        locator: ContractLocator,
        signer: Option<Signer>,
    ) -> ChainResult<Self> {
        let address = TronAddress::try_from(locator.address)?;
        let provider = TronProvider::new(locator.domain.clone(), conf)?;
        let contract = Arc::new(MailboxContract::new(address, provider.eth_client.clone()));

        Ok(TronMailbox {
            contract,
            provider,
            signer,
        })
    }
}

impl HyperlaneChain for TronMailbox {
    fn domain(&self) -> &HyperlaneDomain {
        self.provider.domain()
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        self.provider.provider()
    }
}

impl HyperlaneContract for TronMailbox {
    fn address(&self) -> H256 {
        TronAddress::from(self.contract.address()).into()
    }
}

#[async_trait]
impl Mailbox for TronMailbox {
    #[instrument(skip(self))]
    async fn count(&self, reorg_period: &ReorgPeriod) -> ChainResult<u32> {
        let call =
            call_with_reorg_period(&self.provider, reorg_period, self.contract.nonce()).await?;
        let nonce = call.call().await?;

        Ok(nonce)
    }

    #[instrument(skip(self))]
    async fn delivered(&self, id: H256) -> ChainResult<bool> {
        Ok(self.contract.delivered(id.into()).call().await?)
    }

    #[instrument(skip(self))]
    async fn default_ism(&self) -> ChainResult<H256> {
        let ism: TronAddress = self.contract.default_ism().call().await?.into();

        Ok(ism.into())
    }

    #[instrument(skip(self))]
    async fn recipient_ism(&self, recipient: H256) -> ChainResult<H256> {
        let recipient: TronAddress = recipient.try_into()?;

        let ism: TronAddress = self
            .contract
            .recipient_ism(recipient.into())
            .call()
            .await?
            .into();

        Ok(ism.into())
    }

    #[instrument(skip(self), fields(metadata=%bytes_to_hex(metadata)))]
    async fn process(
        &self,
        message: &HyperlaneMessage,
        metadata: &[u8],
        tx_gas_limit: Option<U256>,
    ) -> ChainResult<TxOutcome> {
        let signer = self
            .signer
            .as_ref()
            .ok_or(ChainCommunicationError::SignerUnavailable)?;

        send_transaction(
            &self.provider,
            &self.contract.address().into(),
            process_calldata(message, metadata),
            signer,
            tx_gas_limit.map(|v| v.as_u64()),
        )
        .await
        .map_err(Into::into)
    }

    #[instrument(skip(self), fields(msg=%message, metadata=%bytes_to_hex(metadata)))]
    async fn process_estimate_costs(
        &self,
        message: &HyperlaneMessage,
        metadata: &[u8],
    ) -> ChainResult<TxCostEstimate> {
        // TODO use correct data upon integrating IGP support
        Ok(TxCostEstimate {
            gas_limit: U256::zero(),
            gas_price: hyperlane_core::FixedPointNumber::zero(),
            l2_gas_limit: None,
        })
    }

    fn process_calldata(&self, message: &HyperlaneMessage, metadata: &[u8]) -> Vec<u8> {
        ethers::abi::AbiEncode::encode(process_calldata(message, metadata))
    }
}

fn process_calldata(message: &HyperlaneMessage, metadata: &[u8]) -> ProcessCall {
    ProcessCall {
        message: RawHyperlaneMessage::from(message).to_vec().into(),
        metadata: metadata.to_vec().into(),
    }
}
