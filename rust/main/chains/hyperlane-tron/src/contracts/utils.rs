use ethers::{
    abi::{Detokenize, RawLog},
    contract::{builders::ContractCall, EthCall, EthEvent, LogMeta as EthersLogMeta},
    providers::Middleware,
    types::H160 as EthersH160,
};
use heliosphere::MethodCall;
use heliosphere_signer::signer::Signer as _;
use tracing::instrument;

use hyperlane_core::{ChainResult, LogMeta, ReorgPeriod, TxOutcome, H256, H512, U256};

use crate::{HyperlaneTronError, Signer, TronAddress, TronProvider};

pub(crate) async fn estimate_energy<T: EthCall>(
    provider: &TronProvider,
    contract: &TronAddress,
    call_args: T,
) -> Result<u64, HyperlaneTronError> {
    let method_call = MethodCall {
        caller: &EthersH160::zero().into(),
        contract: contract.as_ref(),
        selector: &T::abi_signature(),
        parameter: &call_args.encode(),
    };

    provider
        .rpc_client
        .estimate_energy(&method_call)
        .await
        .map_err(Into::<HyperlaneTronError>::into)
}

#[instrument(level = "trace", err, ret, skip(provider))]
pub(crate) async fn get_finalized_block_number(
    provider: &TronProvider,
    reorg_period: &ReorgPeriod,
) -> ChainResult<u32> {
    let number = match reorg_period {
        ReorgPeriod::None | ReorgPeriod::Blocks(_) => {
            let block = provider
                .rpc_client
                .get_latest_block()
                .await
                .map(|blocks| blocks.block_number())
                .map_err(Into::<HyperlaneTronError>::into)?;

            if let ReorgPeriod::Blocks(lag) = reorg_period {
                block.saturating_sub(lag.get() as u64)
            } else {
                block
            }
        }
        ReorgPeriod::Tag(_) => provider
            .rpc_client
            .get_finalized_block_number()
            .await
            .map_err(Into::<HyperlaneTronError>::into)?,
    };

    Ok(number.try_into().unwrap())
}

pub(crate) async fn call_with_reorg_period<M, T>(
    provider: &TronProvider,
    reorg_period: &ReorgPeriod,
    call: ContractCall<M, T>,
) -> ChainResult<ContractCall<M, T>>
where
    T: Detokenize,
{
    if !reorg_period.is_none() {
        let block = get_finalized_block_number(provider, reorg_period).await? as u64;
        Ok(call.block(block))
    } else {
        Ok(call)
    }
}

pub(crate) async fn send_transaction<T: EthCall>(
    provider: &TronProvider,
    contract: &TronAddress,
    call_args: T,
    signer: &Signer,
    energy_limit: Option<u64>,
) -> Result<TxOutcome, HyperlaneTronError> {
    let method_call = MethodCall {
        caller: &signer.0.address(),
        contract: contract.as_ref(),
        selector: &T::abi_signature(),
        parameter: &call_args.encode(),
    };

    let fee_limit = match energy_limit {
        Some(energy_limit) => {
            let energy_price = provider.rpc_client.get_energy_fee().await?;
            Some(energy_limit * energy_price)
        }
        None => None,
    };

    let mut tx = provider
        .rpc_client
        .trigger_contract(&method_call, 0, fee_limit)
        .await?;

    signer.0.sign_transaction(&mut tx)?;

    let txid = provider.rpc_client.broadcast_transaction(&tx).await?;

    let confirmed = provider.rpc_client.await_confirmation(txid).await.is_ok();

    Ok(TxOutcome {
        transaction_id: H256::from(txid.0).into(),
        executed: confirmed,
        // TODO: calculate gas
        gas_used: U256::zero(),
        gas_price: U256::zero().try_into().unwrap(),
    })
}

pub(crate) async fn fetch_raw_logs_and_meta<T: EthEvent>(
    provider: &TronProvider,
    contract_address: EthersH160,
    tx_hash: H512,
) -> ChainResult<Vec<(T, LogMeta)>> {
    let receipt = provider
        .eth_client
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or(HyperlaneTronError::CoreError(
            heliosphere_core::Error::InvalidTransactionId,
        ))?;

    let logs: Vec<(T, LogMeta)> = receipt
        .logs
        .into_iter()
        .filter_map(|log| {
            // Filter out logs that aren't emitted by this contract
            if log.address != contract_address {
                return None;
            }
            let raw_log = RawLog {
                topics: log.topics.clone(),
                data: log.data.to_vec(),
            };
            let log_meta: EthersLogMeta = (&log).into();
            let event_filter = T::decode_log(&raw_log).ok();
            event_filter.map(|log| (log, log_meta.into()))
        })
        .collect();

    Ok(logs)
}
