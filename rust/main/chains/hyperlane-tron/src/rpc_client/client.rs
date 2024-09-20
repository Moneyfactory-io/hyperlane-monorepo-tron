use heliosphere::{Error, RpcClient};
use heliosphere_core::block::Block;
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use url::Url;

pub(crate) struct TronRpcClient(RpcClient);

impl TronRpcClient {
    pub fn new(rpc_endpoint: Url) -> Result<Self, Error> {
        Ok(TronRpcClient(RpcClient::new(rpc_endpoint)?))
    }

    pub async fn get_finalized_block_number(&self) -> Result<u64, Error> {
        let resp: Block = self
            .api_post(
                "/walletsolidity/getblock",
                &serde_json::json!({
                    "detail": false
                }),
            )
            .await?;

        Ok(resp.block_number())
    }

    pub async fn get_energy_fee(&self) -> Result<u64, Error> {
        let params = self.get_chain_parameters().await?;
        params
            .get("getEnergyFee")
            .map(|v| *v as u64)
            .ok_or_else(|| Error::UnknownResponse("getEnergyFee not found".to_owned()))
    }
}

impl Debug for TronRpcClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("RpcClient { ... }")
    }
}

impl Deref for TronRpcClient {
    type Target = RpcClient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
