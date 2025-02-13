import { Address, AddressBase58 } from '@hyperlane-xyz/utils';

export const evmToTronAddressHex = (
  address: Address,
  addressPrefix: string = '41',
): AddressBase58 => {
  const cleanHex = address.replace(/^0x/, '');

  return addressPrefix + cleanHex;
};
