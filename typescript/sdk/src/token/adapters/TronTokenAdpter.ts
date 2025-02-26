import { BigNumber } from 'ethers';
import { Contract, Types } from 'tronweb';
import { encodeParamsV2ByABI } from 'tronweb/utils';
import { call } from 'viem/actions';

import {
  Address,
  Domain,
  Numberish,
  addressToByteHexString,
  addressToBytes32,
  bytes32ToAddress,
  strip0x,
} from '@hyperlane-xyz/utils';

import { BaseTronAdapter } from '../../app/MultiProtocolApp.js';
import { TokenMetadata } from '../types.js';

import {
  IHypTokenAdapter,
  ITokenAdapter,
  InterchainGasQuote,
  TransferParams,
  TransferRemoteParams,
} from './ITokenAdapter.js';

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
    // Not implemented for native token
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
    fromAccountOwner,
  }: TransferParams): Promise<Types.Transaction> {
    if (!fromAccountOwner)
      throw new Error('fromAccountOwner is required for tron transactions');

    const tx = this.getProvider().transactionBuilder.sendTrx(
      recipient,
      Number(weiAmountOrId),
      this.getTronAddress(fromAccountOwner),
    );

    return tx;
  }
}

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
  async _getMethodInstance(method: string) {
    const contract = await this.getContract();

    return contract.methodInstances[method];
  }

  async _getRawParameters(method: string, ...args: any[]) {
    const methodInstance = await this._getMethodInstance(method);

    return encodeParamsV2ByABI(methodInstance.abi, args);
  }

  override async getTotalSupply(): Promise<bigint | undefined> {
    const provider = this.getProvider();

    const contract = await this.getContract();

    const totalSupply = await contract.totalSupply().call();

    return BigInt(provider.toBigNumber(totalSupply).toString(10));
  }

  /**
   * address - evm compatible address
   **/
  override async getBalance(address: Address): Promise<bigint> {
    const provider = this.getProvider();

    const contract = await this.getContract();

    const balance = await contract
      .balanceOf(this.getTronAddress(address))
      .call();

    return BigInt(provider.toBigNumber(balance).toString(10));
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
    const provider = this.getProvider();

    const contract = await this.getContract();

    const allowance = await contract
      .allowance(this.getTronAddress(owner), this.getTronAddress(spender))
      .call();

    return BigNumber.from(
      provider.toBigNumber(allowance.remaining).toString(10),
    ).lt(weiAmountOrId);
  }

  // wanna rewrite it to contract call but now it is imposible
  override async populateApproveTx({
    weiAmountOrId,
    recipient,
    fromAccountOwner,
  }: TransferParams): Promise<Types.Transaction> {
    if (!fromAccountOwner)
      throw new Error('fromAccountOwner is required for tron transactions');

    const provider = this.getProvider();

    const rawParameter = await this._getRawParameters(
      'approve',
      this.getTronAddress(recipient),
      weiAmountOrId,
    );

    const txWrapper = await provider.transactionBuilder.triggerSmartContract(
      this.addresses.token,
      'approve',
      {
        rawParameter,
      },
      [],
      this.getTronAddress(fromAccountOwner!),
    );

    if (!txWrapper.result.result) {
      throw new Error('Error while populating approve tx');
    }

    return txWrapper.transaction;
  }

  override async populateTransferTx({
    recipient,
    weiAmountOrId,
    fromAccountOwner,
  }: TransferParams): Promise<Types.Transaction> {
    if (!fromAccountOwner)
      throw new Error('fromAccountOwner is required for tron transactions');

    const provider = this.getProvider();

    const rawParameter = await this._getRawParameters(
      'transfer',
      this.getTronAddress(recipient),
      weiAmountOrId,
    );

    const txWrapper = await provider.transactionBuilder.triggerSmartContract(
      this.addresses.token,
      'transfer',
      {
        rawParameter,
      },
      [],
      this.getTronAddress(fromAccountOwner!),
    );

    if (!txWrapper.result.result) {
      throw new Error('Error while populating transfer tx');
    }

    return txWrapper.transaction;
  }
}

// Interacts with TRC-721 and TRC-165 NFT contracts
export class TronTRC721TokenAdapter
  extends TronTRC20TokenAdapter
  implements ITokenAdapter<Types.Transaction>
{
  override async getTotalSupply(): Promise<bigint | undefined> {
    // Not implemented for NFT
    return undefined;
  }

  override async populateTransferTx({
    recipient,
    weiAmountOrId, // NFT Id
    fromAccountOwner,
    fromTokenAccount,
  }: TransferParams): Promise<Types.Transaction> {
    if (!fromAccountOwner)
      throw new Error('fromAccountOwner is required for tron transactions');

    if (!fromTokenAccount)
      throw new Error('fromTokenAccount is required for nft transfers');

    const provider = this.getProvider();

    const rawParameter = await this._getRawParameters(
      'transferFrom',
      this.getTronAddress(fromTokenAccount),
      this.getTronAddress(recipient),
      weiAmountOrId,
    );

    const txWrapper = await provider.transactionBuilder.triggerSmartContract(
      this.addresses.token,
      'transferFrom',
      {
        rawParameter,
      },
      [],
      this.getTronAddress(fromAccountOwner!),
    );

    if (!txWrapper.result.result) {
      throw new Error('Error while populating transfer tx');
    }

    return txWrapper.transaction;
  }
}

export class TronHypSyntheticAdapter
  extends TronTRC20TokenAdapter
  implements IHypTokenAdapter<Types.Transaction>
{
  override async isApproveRequired(
    _owner: Address,
    _spender: Address,
    _weiAmountOrId: Numberish,
  ): Promise<boolean> {
    return false;
  }

  async getDomains(): Promise<Domain[]> {
    const contract = await this.getContract();

    return await contract.domains().call();
  }

  async getRouterAddress(domain: Domain): Promise<Buffer> {
    const contract = await this.getContract();

    const routerAddressesAsBytes32 = await contract.routers(domain).call();
    // Evm addresses will be padded with 12 bytes
    if (routerAddressesAsBytes32.startsWith('0x000000000000000000000000')) {
      return Buffer.from(
        strip0x(bytes32ToAddress(routerAddressesAsBytes32)),
        'hex',
      );
      // Otherwise leave the address unchanged
    } else {
      return Buffer.from(strip0x(routerAddressesAsBytes32), 'hex');
    }
  }

  async getAllRouters(): Promise<Array<{ domain: Domain; address: Buffer }>> {
    const domains = await this.getDomains();
    const routers: Buffer[] = await Promise.all(
      domains.map((d) => this.getRouterAddress(d)),
    );
    return domains.map((d, i) => ({ domain: d, address: routers[i] }));
  }

  getBridgedSupply(): Promise<bigint | undefined> {
    return this.getTotalSupply();
  }

  async quoteTransferRemoteGas(
    destination: Domain,
  ): Promise<InterchainGasQuote> {
    const provider = this.getProvider();

    const contract = await this.getContract();

    const gasPayment = await contract.quoteGasPayment(destination);
    // If Tron hyp contracts eventually support alternative IGP tokens,
    // this would need to determine the correct token address
    return {
      amount: BigInt(provider.toBigNumber(gasPayment).toString(10)),
    };
  }

  async populateTransferRemoteTx({
    weiAmountOrId,
    destination,
    recipient,
    interchainGas,
    fromAccountOwner,
  }: TransferRemoteParams): Promise<Types.Transaction> {
    if (!fromAccountOwner)
      throw new Error('fromAccountOwner is required for tron transactions');

    if (!interchainGas)
      interchainGas = await this.quoteTransferRemoteGas(destination);

    const provider = this.getProvider();

    const recipBytes32 = addressToBytes32(addressToByteHexString(recipient));

    const rawParameter = await this._getRawParameters(
      'transferRemote',
      destination,
      recipBytes32,
      weiAmountOrId,
    );

    const txWrapper = await provider.transactionBuilder.triggerSmartContract(
      this.addresses.token,
      'transferRemote',
      {
        rawParameter,
        feeLimit: +interchainGas!.amount.toString(),
      },
      [],
      this.getTronAddress(fromAccountOwner!),
    );

    if (!txWrapper.result.result) {
      throw new Error('Error while populating transfer tx');
    }

    return txWrapper.transaction;
  }
}
