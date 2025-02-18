import { Contract, Types } from 'tronweb';

import { Address } from '@hyperlane-xyz/utils';

import { BaseTronAdapter } from '../../app/MultiProtocolApp.js';
import { evmToTronAddressHex } from '../../utils/tron.js';
import { TokenMetadata } from '../types.js';

import { ITokenAdapter, TransferParams } from './ITokenAdapter.js';

export const DEFAULT_TRON_ADDRESS_PREFIX: string = '41';

export class TronNativeTokenAdapter
  extends BaseTronAdapter
  implements ITokenAdapter<Types.Transaction>
{
  getTronAddress(address: Address): Address {
    const provider = this.getProvider();

    const tronHex = provider.address.toHex(address);

    // tronlike address
    return provider.address.fromHex(tronHex);
  }

  /**
   * address - evm compatible address
   **/
  async getBalance(address: Address): Promise<bigint> {
    const balance = await this.getProvider().trx.getBalance(
      this.getTronAddress(address),
    );

    return BigInt(balance);
  }

  async getTotalSupply(): Promise<bigint | undefined> {
    // Not implemented.
    return undefined;
  }

  getMetadata(): Promise<TokenMetadata> {
    throw new Error('Metadata not available to native tokes.');
  }

  async getMinimumTransferAmount(_recipient: Address): Promise<bigint> {
    return 0n;
  }

  async isApproveRequired(): Promise<boolean> {
    return false;
  }

  populateApproveTx(
    _transferParams: TransferParams,
  ): Promise<Types.Transaction> {
    throw new Error('Approve not required for native tokens.');
  }

  async populateTransferTx({
    recipient,
    weiAmountOrId,
  }: TransferParams): Promise<Types.Transaction> {
    const tx = this.getProvider().transactionBuilder.sendTrx(
      recipient,
      Number(weiAmountOrId),
    );

    this.getProvider().contract().asd();
    return tx;
  }
}

// probably change Types.Transaction to Method interface
// Interacts with TRC-20 contracts
export class TronTRC20TokenAdapter
  extends TronNativeTokenAdapter
  implements ITokenAdapter<Types.Transaction>
{
  public contract: Contract | null = null;

  async init() {
    if (this.contract) return;

    try {
      this.contract = await this.getProvider()
        .contract()
        .at(this.addresses.token);
    } catch {
      throw new Error('Failed to initialize token contract');
    }
  }

  async getContract() {
    if (!this.contract) {
      await this.init();
    }

    return this.contract as Contract;
  }

  /**
   * address - evm compatible address
   **/
  override async getBalance(address: Address): Promise<bigint> {
    const contract = await this.getContract();

    return BigInt(contract.getBalance(this.getTronAddress(address)));
  }
}
