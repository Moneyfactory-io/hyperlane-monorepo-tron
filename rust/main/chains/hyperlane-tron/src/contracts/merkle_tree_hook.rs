use std::fmt::Debug;
use std::ops::RangeInclusive;
use std::sync::Arc;

use async_trait::async_trait;
use tracing::instrument;

use hyperlane_core::{
    accumulator::incremental::IncrementalMerkle, rpc_clients::call_and_retry_indefinitely,
    ChainResult, Checkpoint, ContractLocator, HyperlaneChain, HyperlaneContract, HyperlaneDomain,
    HyperlaneProvider, Indexed, Indexer, LogMeta, MerkleTreeHook, MerkleTreeInsertion, ReorgPeriod,
    SequenceAwareIndexer, H256, H512,
};

use crate::interfaces::merkle_tree_hook::{
    InsertedIntoTreeFilter, MerkleTreeHook as MerkleTreeHookContract, Tree,
};
use crate::{ConnectionConf, TronAddress, TronEthClient, TronProvider};

use super::utils::{call_with_reorg_period, fetch_raw_logs_and_meta, get_finalized_block_number};

/// Struct that retrieves event data for an Tron MerkleTreeHook
#[derive(Debug)]
pub struct TronMerkleTreeHookIndexer {
    contract: Arc<MerkleTreeHookContract<TronEthClient>>,
    provider: TronProvider,
    reorg_period: ReorgPeriod,
}

impl TronMerkleTreeHookIndexer {
    pub fn new(
        conf: ConnectionConf,
        locator: ContractLocator,
        reorg_period: ReorgPeriod,
    ) -> ChainResult<Self> {
        let address = TronAddress::try_from(locator.address)?;
        let provider = TronProvider::new(locator.domain.clone(), conf)?;
        let contract = Arc::new(MerkleTreeHookContract::new(
            address,
            provider.eth_client.clone(),
        ));

        Ok(TronMerkleTreeHookIndexer {
            contract,
            provider,
            reorg_period,
        })
    }
}

#[async_trait]
impl Indexer<MerkleTreeInsertion> for TronMerkleTreeHookIndexer {
    #[instrument(err, skip(self))]
    async fn fetch_logs_in_range(
        &self,
        range: RangeInclusive<u32>,
    ) -> ChainResult<Vec<(Indexed<MerkleTreeInsertion>, LogMeta)>> {
        let events = self
            .contract
            .inserted_into_tree_filter()
            .from_block(*range.start())
            .to_block(*range.end())
            .query_with_meta()
            .await?;

        let logs = events
            .into_iter()
            .map(|(log, log_meta)| {
                (
                    MerkleTreeInsertion::new(log.index, H256::from(log.message_id)).into(),
                    log_meta.into(),
                )
            })
            .collect();
        Ok(logs)
    }

    #[instrument(level = "debug", err, skip(self))]
    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        get_finalized_block_number(&self.provider, &self.reorg_period).await
    }

    async fn fetch_logs_by_tx_hash(
        &self,
        tx_hash: H512,
    ) -> ChainResult<Vec<(Indexed<MerkleTreeInsertion>, LogMeta)>> {
        let raw_logs_and_meta = call_and_retry_indefinitely(|| {
            let provider = self.provider.clone();
            let contract = self.contract.address();
            Box::pin(async move {
                fetch_raw_logs_and_meta::<InsertedIntoTreeFilter>(&provider, contract, tx_hash)
                    .await
            })
        })
        .await;

        let logs = raw_logs_and_meta
            .into_iter()
            .map(|(log, log_meta)| {
                (
                    MerkleTreeInsertion::new(log.index, H256::from(log.message_id)).into(),
                    log_meta,
                )
            })
            .collect();

        Ok(logs)
    }
}

#[async_trait]
impl SequenceAwareIndexer<MerkleTreeInsertion> for TronMerkleTreeHookIndexer {
    async fn latest_sequence_count_and_tip(&self) -> ChainResult<(Option<u32>, u32)> {
        let tip = self.get_finalized_block_number().await?;
        let sequence = self.contract.count().block(u64::from(tip)).call().await?;
        Ok((Some(sequence), tip))
    }
}

#[derive(Debug)]
pub struct TronMerkleTreeHook {
    provider: TronProvider,
    contract: Arc<MerkleTreeHookContract<TronEthClient>>,
}

impl TronMerkleTreeHook {
    pub fn new(conf: ConnectionConf, locator: ContractLocator) -> ChainResult<Self> {
        let address = TronAddress::try_from(locator.address)?;
        let provider = TronProvider::new(locator.domain.clone(), conf)?;
        let contract = Arc::new(MerkleTreeHookContract::new(
            address,
            provider.eth_client.clone(),
        ));

        Ok(TronMerkleTreeHook { provider, contract })
    }
}

impl HyperlaneContract for TronMerkleTreeHook {
    fn address(&self) -> H256 {
        self.contract.address().into()
    }
}

impl HyperlaneChain for TronMerkleTreeHook {
    fn domain(&self) -> &HyperlaneDomain {
        self.provider.domain()
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        self.provider.provider()
    }
}

#[async_trait]
impl MerkleTreeHook for TronMerkleTreeHook {
    #[instrument(skip(self))]
    async fn latest_checkpoint(&self, reorg_period: &ReorgPeriod) -> ChainResult<Checkpoint> {
        let call = call_with_reorg_period(
            &self.provider,
            reorg_period,
            self.contract.latest_checkpoint(),
        )
        .await?;

        let (root, index) = call.call().await?;
        Ok(Checkpoint {
            merkle_tree_hook_address: self.address(),
            mailbox_domain: self.domain().id(),
            root: root.into(),
            index,
        })
    }

    #[instrument(skip(self))]
    #[allow(clippy::needless_range_loop)]
    async fn tree(&self, reorg_period: &ReorgPeriod) -> ChainResult<IncrementalMerkle> {
        let call =
            call_with_reorg_period(&self.provider, reorg_period, self.contract.tree()).await?;

        let tree = call.call().await?.into();
        Ok(tree)
    }

    #[instrument(skip(self))]
    async fn count(&self, reorg_period: &ReorgPeriod) -> ChainResult<u32> {
        let call =
            call_with_reorg_period(&self.provider, reorg_period, self.contract.count()).await?;

        let count = call.call().await?;
        Ok(count)
    }
}

// We don't need the reverse of this impl, so it's ok to disable the clippy lint
#[allow(clippy::from_over_into)]
impl Into<IncrementalMerkle> for Tree {
    fn into(self) -> IncrementalMerkle {
        let branch = self
            .branch
            .iter()
            .map(|v| v.into())
            .collect::<Vec<_>>()
            // we're iterating over a fixed-size array and want to collect into a
            // fixed-size array of the same size (32), so this is safe
            .try_into()
            .unwrap();
        IncrementalMerkle::new(branch, self.count.as_usize())
    }
}
