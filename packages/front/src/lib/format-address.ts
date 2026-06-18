/**
 * 将长地址（如 EVM 钱包地址）缩写为「前 6 位…后 4 位」形式。
 * 长度小于等于 12 的地址原样返回，避免过度截断短标识。
 */
export function formatShortAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}
