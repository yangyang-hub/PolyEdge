import { fetchWriteContract, randomUUID } from "@/lib/api/base";
import type { EncryptedWalletSecretInput } from "@/lib/contracts/dto";

type WalletImportContext = { data: { context_id: string; key_id: string; algorithm: "RSA-OAEP-256+A256GCM"; aad_version: "polyedge-wallet-import-v1"; public_key: JsonWebKey } };
const bytesToBase64Url = (bytes: Uint8Array) => {
  let binary = "";
  bytes.forEach((byte) => { binary += String.fromCharCode(byte); });
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
};

function walletImportAad(contextId: string, binding: string): Uint8Array {
  const encoder = new TextEncoder();
  return domainAad([
    encoder.encode("polyedge-wallet-import-v1"),
    uuidBytes(contextId),
    encoder.encode(binding),
  ]);
}

function domainAad(parts: Uint8Array[]): Uint8Array {
  const size = parts.reduce((total, part) => total + 8 + part.byteLength, 0);
  const output = new Uint8Array(size);
  const view = new DataView(output.buffer);
  let offset = 0;
  for (const part of parts) {
    view.setBigUint64(offset, BigInt(part.byteLength), false);
    offset += 8;
    output.set(part, offset);
    offset += part.byteLength;
  }
  return output;
}

function uuidBytes(value: string): Uint8Array {
  const hex = value.replaceAll("-", "");
  if (!/^[0-9a-fA-F]{32}$/.test(hex)) throw new Error("钱包导入上下文 UUID 无效");
  return Uint8Array.from({ length: 16 }, (_, index) => Number.parseInt(hex.slice(index * 2, index * 2 + 2), 16));
}

export async function encryptWalletSecret(userId: number, secret: { private_key: string; api_key?: string; api_secret?: string; api_passphrase?: string }): Promise<EncryptedWalletSecretInput> {
  const context = await fetchWriteContract<WalletImportContext>("/api/v1/security/wallet-import-contexts", { body: {}, idempotencyKey: randomUUID() });
  if (context.data.aad_version !== "polyedge-wallet-import-v1") throw new Error("不支持的钱包导入 AAD 版本");
  const publicKey = await crypto.subtle.importKey("jwk", context.data.public_key, { name: "RSA-OAEP", hash: "SHA-256" }, false, ["encrypt"]);
  const dataKey = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, true, ["encrypt"]);
  const nonce = crypto.getRandomValues(new Uint8Array(12));
  const plaintext = new TextEncoder().encode(JSON.stringify(secret));
  const aad = walletImportAad(context.data.context_id, String(userId));
  let ciphertext: ArrayBuffer;
  try {
    ciphertext = await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce, additionalData: aad.buffer as ArrayBuffer }, dataKey, plaintext);
  } finally {
    plaintext.fill(0);
  }
  const rawKey = new Uint8Array(await crypto.subtle.exportKey("raw", dataKey));
  let wrappedKey: ArrayBuffer;
  try {
    wrappedKey = await crypto.subtle.encrypt({ name: "RSA-OAEP" }, publicKey, rawKey);
  } finally {
    rawKey.fill(0);
  }
  return { context_id: context.data.context_id, key_id: context.data.key_id, algorithm: context.data.algorithm, wrapped_key: bytesToBase64Url(new Uint8Array(wrappedKey)), nonce: bytesToBase64Url(nonce), ciphertext: bytesToBase64Url(new Uint8Array(ciphertext)) };
}
