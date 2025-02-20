import { BigNumber } from 'ethers';
import { Contract, Types } from 'tronweb';

import { Address, Numberish } from '@hyperlane-xyz/utils';

import { BaseTronAdapter } from '../../app/MultiProtocolApp.js';
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

  async isApproveRequired(
    _owner: Address,
    _spender: Address,
    _weiAmountOrId: Numberish,
  ): Promise<boolean> {
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

  // TODO: Get method params from instances
  // private _getMethodInstance(method: ) {
  //
  // }
  //
  // private _getRawParameters(method: ) {
  //
  // }

  /**
   * address - evm compatible address
   **/
  override async getBalance(address: Address): Promise<bigint> {
    const contract = await this.getContract();

    return BigInt(contract.getBalance(this.getTronAddress(address)));
  }

  // TODO: check if we should check nft
  override async getMetadata(): Promise<TokenMetadata> {
    const issuedTokens = await this.getProvider().trx.getTokensIssuedByAddress(
      this.addresses.token,
    );
    const token = Object.values(issuedTokens).at(0);

    if (!token) {
      throw new Error("Can't get metadata for token");
    }

    return {
      decimals: token.precision,
      symbol: token.abbr,
      name: token.name,
      totalSupply: token.total_supply.toString(),
    };
  }

  override async isApproveRequired(
    owner: Address,
    spender: Address,
    weiAmountOrId: Numberish,
  ): Promise<boolean> {
    const contract = await this.getContract();

    const allowance: bigint = await contract.allowance(
      this.getTronAddress(owner),
      this.getTronAddress(spender),
    );

    return BigNumber.from(allowance).lt(weiAmountOrId);
  }

  // wanna rewrite it to contract call but now it imposible
  override populateApproveTx({
    weiAmountOrId,
    recipient,
  }: TransferParams): Promise<Types.Transaction> {
    const provider = this.getProvider();

    return provider.transactionBuilder.triggerSmartContract(
      this.addresses.token,
      'approve',
      {},
      [{}],
    );
  }
}
