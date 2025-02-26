use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use ethers::{
    providers::{Http, Middleware, Provider},
    types::H160,
};
use tracing::instrument;

use hyperlane_core::{
    BlockInfo, ChainCommunicationError, ChainInfo, ChainResult, HyperlaneChain, HyperlaneDomain,
    HyperlaneProvider, TxnInfo, H256, H512, U256,
};

use crate::{ConnectionConf, HyperlaneTronError, TronRpcClient};

pub(crate) type TronEthClient = Provider<Http>;

/// Abstraction over a connection to a Tron chain
#[derive(Clone, Debug)]
pub struct TronProvider {
    domain: HyperlaneDomain,
    pub(crate) eth_client: Arc<TronEthClient>,
    pub(crate) rpc_client: Arc<TronRpcClient>,
}

impl TronProvider {
    pub fn new(domain: HyperlaneDomain, conf: ConnectionConf) -> Result<Self, HyperlaneTronError> {
        Ok(TronProvider {
            domain,
            eth_client: Arc::new(Provider::new(Http::new(conf.url.clone()))),
            rpc_client: Arc::new(TronRpcClient::new(conf.url)?),
        })
    }
}

impl HyperlaneChain for TronProvider {
    fn domain(&self) -> &HyperlaneDomain {
        &self.domain
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        Box::new(TronProvider {
            domain: self.domain.clone(),
            eth_client: self.eth_client.clone(),
            rpc_client: self.rpc_client.clone(),
        })
    }
}

#[async_trait]
impl HyperlaneProvider for TronProvider {
    #[instrument(err, skip(self))]
    async fn get_txn_by_hash(&self, hash: &H512) -> ChainResult<TxnInfo> {
        todo!()
    }

    #[instrument(err, skip(self))]
    async fn is_contract(&self, address: &H256) -> ChainResult<bool> {
        let code = self
            .eth_client
            .get_code(H160::from(*address), None)
            .await
            .map_err(ChainCommunicationError::from_other)?;
        Ok(!code.is_empty())
    }

    #[instrument(err, skip(self))]
    async fn get_balance(&self, address: String) -> ChainResult<U256> {
        let address = &address.parse().map_err(Into::<HyperlaneTronError>::into)?;

        let balance = self
            .rpc_client
            .get_account_balance(address)
            .await
            .map_err(Into::<HyperlaneTronError>::into)?
            .into();

        Ok(balance)
    }

    async fn get_chain_metrics(&self) -> ChainResult<Option<ChainInfo>> {
        todo!()
    }

    async fn get_block_by_height(&self, height: u64) -> ChainResult<BlockInfo> {
        todo!()
    }
}
