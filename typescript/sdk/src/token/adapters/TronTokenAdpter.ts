import { Types } from 'tronweb';

import { Address, Numberish } from '@hyperlane-xyz/utils';

import { BaseTronAdapter } from '../../app/MultiProtocolApp.js';
import { evmToTronAddressHex } from '../../utils/tron.js';

import { ITokenAdapter, TransferParams } from './ITokenAdapter.js';

export const DEFAULT_TRON_ADDRESS_PREFIX: string = '41';

export class TronNativeTokenAdapter
  extends BaseTronAdapter
  implements ITokenAdapter<Types.Transaction>
{
  // evm compatible address
  async getBalance(address: Address): Promise<bigint> {
    // TODO: Add address prefix support
    const tronAddress = this.getProvider().address.toHex(
      evmToTronAddressHex(address),
    );

    const balance = await this.getProvider().trx.getBalance(tronAddress);

    return BigInt(balance);
  }
  getTotalSupply(): Promise<bigint | undefined> {
    throw new Error('Method not implemented.');
  }
  getMetadata(
    isNft?: boolean | undefined,
  ): Promise<{
    symbol: string;
    name: string;
    totalSupply: string | number;
    decimals?: number | undefined;
    scale?: number | undefined;
    isNft?: boolean | undefined;
  }> {
    throw new Error('Method not implemented.');
  }
  getMinimumTransferAmount(recipient: string): Promise<bigint> {
    throw new Error('Method not implemented.');
  }
  isApproveRequired(
    owner: string,
    spender: string,
    weiAmountOrId: Numberish,
  ): Promise<boolean> {
    throw new Error('Method not implemented.');
  }
  populateApproveTx(
    params: TransferParams,
  ): Promise<Types.Transaction<Types.ContractParamter>> {
    throw new Error('Method not implemented.');
  }
  populateTransferTx(
    params: TransferParams,
  ): Promise<Types.Transaction<Types.ContractParamter>> {
    throw new Error('Method not implemented.');
  }
}
