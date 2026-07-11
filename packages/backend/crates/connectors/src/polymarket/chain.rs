const POLYGON_RPC_TIMEOUT: Duration = Duration::from_secs(15);
const POLYGON_CHAIN_ID: u64 = 137;
const POLYMARKET_BRIDGE_BASE_URL: &str = "https://bridge.polymarket.com";
const POLYMARKET_PUSD_CONTRACT_ADDRESS: &str = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB";
const POLYMARKET_CONDITIONAL_TOKENS_ADDRESS: &str =
    "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045";
const ERC20_BALANCE_OF_SELECTOR: &str = "70a08231";
const ERC20_TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
const ERC20_DECIMALS: u32 = 6;
const CTF_SHARE_DECIMALS: u8 = 6;
const SAFE_GET_OWNERS_SELECTOR: [u8; 4] = [0xa0, 0xe6, 0x7e, 0x2b];
const SAFE_GET_THRESHOLD_SELECTOR: [u8; 4] = [0xe7, 0x52, 0x35, 0xb8];
const SAFE_NONCE_SELECTOR: [u8; 4] = [0xaf, 0xfe, 0xd0, 0xe0];
const SAFE_GET_TRANSACTION_HASH_SELECTOR: [u8; 4] = [0xd8, 0xd1, 0x1f, 0x78];
const SAFE_EXEC_TRANSACTION_SELECTOR: [u8; 4] = [0x6a, 0x76, 0x12, 0x02];

#[derive(Debug, Clone, Copy)]
pub struct PolymarketFundingToken {
    pub id: &'static str,
    pub symbol: &'static str,
    pub name: &'static str,
    pub address: &'static str,
    pub decimals: u8,
    pub min_checkout_usd: Decimal,
}

#[derive(Debug, Clone)]
pub struct PolymarketFundingTransferRequest {
    pub polymarket_wallet_address: String,
    pub token_id: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct PolymarketFundingTransferReceipt {
    pub tx_hash: String,
    pub source_address: String,
    pub polymarket_wallet_address: String,
    pub bridge_deposit_address: String,
    pub token: PolymarketFundingToken,
    pub amount: Decimal,
    pub amount_units: String,
}

#[derive(Debug, Clone)]
pub struct PolymarketMergePositionsRequest {
    pub proxy_wallet_address: String,
    pub condition_id: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct PolymarketMergePositionsReceipt {
    pub tx_hash: String,
    pub owner_address: String,
    pub proxy_wallet_address: String,
    pub condition_id: String,
    pub amount: Decimal,
    pub amount_units: String,
    pub safe_nonce: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolymarketTransactionReceiptStatus {
    Pending,
    Succeeded,
    Reverted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolymarketTransactionReceipt {
    pub tx_hash: String,
    pub status: PolymarketTransactionReceiptStatus,
    pub block_number: Option<String>,
}

const POLYGON_FUNDING_TOKENS: &[PolymarketFundingToken] = &[
    PolymarketFundingToken {
        id: "usdc",
        symbol: "USDC",
        name: "USD Coin",
        address: "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
        decimals: 6,
        min_checkout_usd: Decimal::TWO,
    },
    PolymarketFundingToken {
        id: "usdt",
        symbol: "USDT0",
        name: "Polygon USDT0",
        address: "0xc2132D05D31c914a87C6611C10748AEb04B58e8F",
        decimals: 6,
        min_checkout_usd: Decimal::TWO,
    },
];

#[derive(Debug, Clone)]
pub struct PolymarketChainConnector {
    rpc_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcTransactionReceiptResponse {
    result: Option<JsonRpcTransactionReceipt>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcTransactionReceipt {
    transaction_hash: String,
    status: String,
    block_number: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct BridgeDepositResponse {
    address: BridgeDepositAddresses,
}

#[derive(Debug, serde::Deserialize)]
struct BridgeDepositAddresses {
    evm: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct BridgeSupportedAssetsResponse {
    #[serde(rename = "supportedAssets")]
    supported_assets: Vec<BridgeSupportedAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct BridgeSupportedAsset {
    #[serde(rename = "chainId")]
    chain_id: String,
    token: BridgeSupportedToken,
    #[serde(rename = "minCheckoutUsd")]
    min_checkout_usd: Decimal,
}

#[derive(Debug, serde::Deserialize)]
struct BridgeSupportedToken {
    address: String,
    decimals: u8,
}

impl PolymarketChainConnector {
    pub fn new(rpc_url: &str) -> Result<Self> {
        let rpc_url = normalize_optional_text(Some(rpc_url.to_string())).ok_or_else(|| {
            AppError::invalid_input(
                "POLYGON_RPC_URL_REQUIRED",
                "polygon rpc url must not be empty",
            )
        })?;
        let client = reqwest::Client::builder()
            .timeout(POLYGON_RPC_TIMEOUT)
            .build()
            .map_err(|error| {
                AppError::internal(
                    "POLYGON_RPC_CLIENT_INIT_FAILED",
                    format!("failed to initialize Polygon RPC client: {error}"),
                )
            })?;
        Ok(Self { rpc_url, client })
    }

    pub async fn fetch_pusd_balance(&self, wallet_address: &str) -> Result<Decimal> {
        self.fetch_erc20_balance(POLYMARKET_PUSD_CONTRACT_ADDRESS, wallet_address, ERC20_DECIMALS)
            .await
    }

    pub async fn fetch_funding_token_balance(
        &self,
        token_id: &str,
        wallet_address: &str,
    ) -> Result<Decimal> {
        let token = funding_token_by_id(token_id)?;
        self.fetch_erc20_balance(token.address, wallet_address, u32::from(token.decimals))
            .await
    }

    #[must_use]
    pub fn polygon_funding_tokens() -> &'static [PolymarketFundingToken] {
        POLYGON_FUNDING_TOKENS
    }

    pub fn funding_source_address(private_key: &str, chain_id: u64) -> Result<String> {
        let signer = funding_private_key_signer(private_key, chain_id)?;
        Ok(format!("{:#x}", signer.address()))
    }

    pub fn normalize_funding_wallet_address(value: &str) -> Result<String> {
        normalize_evm_address(
            "polymarket_wallet_address",
            value,
            "POLYMARKET_FUNDING_WALLET_ADDRESS_INVALID",
        )
    }

    pub async fn submit_funding_transfer(
        &self,
        private_key: &str,
        chain_id: u64,
        request: PolymarketFundingTransferRequest,
    ) -> Result<PolymarketFundingTransferReceipt> {
        if chain_id != POLYGON_CHAIN_ID {
            return Err(AppError::invalid_input(
                "POLYGON_FUNDING_CHAIN_UNSUPPORTED",
                format!("funding transfers require Polygon chain_id={POLYGON_CHAIN_ID}, got {chain_id}"),
            ));
        }

        let token = funding_token_by_id(&request.token_id)?;
        let polymarket_wallet_address =
            Self::normalize_funding_wallet_address(&request.polymarket_wallet_address)?;
        let bridge_asset = self.bridge_supported_asset_for_token(token).await?;
        if bridge_asset.token.decimals != token.decimals {
            return Err(AppError::dependency_unavailable(
                "POLYMARKET_BRIDGE_TOKEN_DECIMALS_MISMATCH",
                format!(
                    "Polymarket Bridge reports {} decimals for {}, expected {}",
                    bridge_asset.token.decimals, token.symbol, token.decimals
                ),
            ));
        }
        if request.amount < bridge_asset.min_checkout_usd {
            return Err(AppError::invalid_input(
                "POLYMARKET_BRIDGE_AMOUNT_BELOW_MINIMUM",
                format!(
                    "{} minimum checkout amount is {} USD",
                    token.symbol, bridge_asset.min_checkout_usd
                ),
            ));
        }

        let bridge_deposit_address = self
            .fetch_bridge_deposit_address(&polymarket_wallet_address)
            .await?;
        let token_address = parse_alloy_address(
            "token_address",
            token.address,
            "POLYGON_FUNDING_TOKEN_ADDRESS_INVALID",
        )?;
        let bridge_deposit_address_parsed = parse_alloy_address(
            "bridge_deposit_address",
            &bridge_deposit_address,
            "POLYMARKET_BRIDGE_DEPOSIT_ADDRESS_INVALID",
        )?;
        let amount_units = decimal_amount_to_units(request.amount, token.decimals)?;
        let signer = funding_private_key_signer(private_key, chain_id)?;
        let source_address = format!("{:#x}", signer.address());
        let provider = ProviderBuilder::new()
            .with_chain_id(chain_id)
            .wallet(signer)
            .connect_http(self.rpc_url.parse().map_err(|error| {
                AppError::invalid_input(
                    "POLYGON_RPC_URL_INVALID",
                    format!("invalid Polygon RPC URL: {error}"),
                )
            })?);
        let data = build_erc20_transfer_calldata(bridge_deposit_address_parsed, amount_units);
        let tx = AlloyTransactionRequest::default()
            .with_to(token_address)
            .with_input(AlloyBytes::from(data));
        let pending = provider.send_transaction(tx).await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYGON_FUNDING_SEND_FAILED",
                format!("failed to broadcast Polygon funding transfer: {error}"),
            )
        })?;
        let tx_hash = format!("{:#x}", pending.tx_hash());

        Ok(PolymarketFundingTransferReceipt {
            tx_hash,
            source_address,
            polymarket_wallet_address,
            bridge_deposit_address,
            token,
            amount: request.amount,
            amount_units: amount_units.to_string(),
        })
    }

    pub async fn submit_merge_positions(
        &self,
        private_key: &str,
        chain_id: u64,
        request: PolymarketMergePositionsRequest,
    ) -> Result<PolymarketMergePositionsReceipt> {
        if chain_id != POLYGON_CHAIN_ID {
            return Err(AppError::invalid_input(
                "POLYMARKET_MERGE_CHAIN_UNSUPPORTED",
                format!("CTF merge requires Polygon chain_id={POLYGON_CHAIN_ID}, got {chain_id}"),
            ));
        }

        let proxy_wallet_address = normalize_evm_address(
            "proxy_wallet_address",
            &request.proxy_wallet_address,
            "POLYMARKET_PROXY_WALLET_ADDRESS_INVALID",
        )?;
        let safe_address = parse_alloy_address(
            "proxy_wallet_address",
            &proxy_wallet_address,
            "POLYMARKET_PROXY_WALLET_ADDRESS_INVALID",
        )?;
        let conditional_tokens_address = parse_alloy_address(
            "conditional_tokens_address",
            POLYMARKET_CONDITIONAL_TOKENS_ADDRESS,
            "POLYMARKET_CONDITIONAL_TOKENS_ADDRESS_INVALID",
        )?;
        let collateral_address = parse_alloy_address(
            "collateral_address",
            POLYMARKET_PUSD_CONTRACT_ADDRESS,
            "POLYMARKET_COLLATERAL_ADDRESS_INVALID",
        )?;
        let condition_id = parse_bytes32(
            "condition_id",
            &request.condition_id,
            "POLYMARKET_MERGE_CONDITION_ID_INVALID",
        )?;
        let amount_units = decimal_amount_to_units(request.amount, CTF_SHARE_DECIMALS)?;
        let signer = funding_private_key_signer(private_key, chain_id)?;
        let owner_address = signer.address();
        self.ensure_safe_owner_threshold_one(&proxy_wallet_address, owner_address)
            .await?;
        let safe_nonce = self.safe_nonce(&proxy_wallet_address).await?;

        let merge_data = build_ctf_merge_positions_calldata(
            collateral_address,
            condition_id,
            amount_units,
        );
        let safe_tx_hash_call = build_safe_get_transaction_hash_calldata(
            conditional_tokens_address,
            AlloyU256::ZERO,
            &merge_data,
            0,
            AlloyU256::ZERO,
            AlloyU256::ZERO,
            AlloyU256::ZERO,
            AlloyAddress::ZERO,
            AlloyAddress::ZERO,
            safe_nonce,
        );
        let safe_tx_hash = self
            .eth_call(&proxy_wallet_address, &safe_tx_hash_call)
            .await
            .and_then(|bytes| decode_bytes32_result(&bytes, "POLYMARKET_SAFE_HASH_INVALID"))?;
        let signature = signer.sign_hash(&AlloyB256::from(safe_tx_hash)).await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_SAFE_SIGN_FAILED",
                format!("failed to sign Safe transaction hash: {error}"),
            )
        })?;
        let exec_data = build_safe_exec_transaction_calldata(
            conditional_tokens_address,
            AlloyU256::ZERO,
            &merge_data,
            0,
            AlloyU256::ZERO,
            AlloyU256::ZERO,
            AlloyU256::ZERO,
            AlloyAddress::ZERO,
            AlloyAddress::ZERO,
            &signature.as_bytes(),
        );
        let provider = ProviderBuilder::new()
            .with_chain_id(chain_id)
            .wallet(signer)
            .connect_http(self.rpc_url.parse().map_err(|error| {
                AppError::invalid_input(
                    "POLYGON_RPC_URL_INVALID",
                    format!("invalid Polygon RPC URL: {error}"),
                )
            })?);
        let tx = AlloyTransactionRequest::default()
            .with_to(safe_address)
            .with_input(AlloyBytes::from(exec_data));
        let pending = provider.send_transaction(tx).await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYMARKET_MERGE_SEND_FAILED",
                format!("failed to broadcast Safe merge transaction: {error}"),
            )
        })?;
        Ok(PolymarketMergePositionsReceipt {
            tx_hash: format!("{:#x}", pending.tx_hash()),
            owner_address: format!("{:#x}", owner_address),
            proxy_wallet_address,
            condition_id: request.condition_id,
            amount: request.amount,
            amount_units: amount_units.to_string(),
            safe_nonce: safe_nonce.to_string(),
        })
    }

    /// Query a Polygon transaction without ever rebroadcasting it.
    ///
    /// `eth_getTransactionReceipt` returns `null` while a transaction is not
    /// mined (or is not yet visible to the selected RPC). Callers must treat
    /// that state as unresolved instead of assuming the transaction is absent.
    pub async fn fetch_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<PolymarketTransactionReceipt> {
        let tx_hash = normalize_transaction_hash(tx_hash)?;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash]
        });
        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYGON_RECEIPT_REQUEST_FAILED",
                    format!("failed to query Polygon transaction receipt: {error}"),
                )
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYGON_RECEIPT_STATUS_FAILED",
                    format!("Polygon receipt RPC returned error status: {error}"),
                )
            })?;
        let payload = response
            .json::<JsonRpcTransactionReceiptResponse>()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYGON_RECEIPT_DECODE_FAILED",
                    format!("failed to decode Polygon transaction receipt: {error}"),
                )
            })?;
        if let Some(error) = payload.error {
            return Err(AppError::dependency_unavailable(
                "POLYGON_RECEIPT_RPC_ERROR",
                format!("Polygon RPC error {}: {}", error.code, error.message),
            ));
        }
        let Some(receipt) = payload.result else {
            return Ok(PolymarketTransactionReceipt {
                tx_hash,
                status: PolymarketTransactionReceiptStatus::Pending,
                block_number: None,
            });
        };
        let receipt_hash = normalize_transaction_hash(&receipt.transaction_hash)?;
        if !receipt_hash.eq_ignore_ascii_case(&tx_hash) {
            return Err(AppError::dependency_unavailable(
                "POLYGON_RECEIPT_HASH_MISMATCH",
                format!(
                    "Polygon receipt hash {receipt_hash} does not match requested hash {tx_hash}"
                ),
            ));
        }
        let status = match receipt.status.trim().to_ascii_lowercase().as_str() {
            "0x1" | "0x01" => PolymarketTransactionReceiptStatus::Succeeded,
            "0x0" | "0x00" => PolymarketTransactionReceiptStatus::Reverted,
            other => {
                return Err(AppError::dependency_unavailable(
                    "POLYGON_RECEIPT_EXECUTION_STATUS_INVALID",
                    format!("Polygon receipt returned unsupported status {other}"),
                ));
            }
        };
        let block_number = receipt
            .block_number
            .filter(|value| !value.trim().is_empty());
        if block_number.is_none() {
            return Err(AppError::dependency_unavailable(
                "POLYGON_RECEIPT_BLOCK_NUMBER_MISSING",
                "mined Polygon transaction receipt did not include blockNumber",
            ));
        }
        Ok(PolymarketTransactionReceipt {
            tx_hash,
            status,
            block_number,
        })
    }

    async fn bridge_supported_asset_for_token(
        &self,
        token: PolymarketFundingToken,
    ) -> Result<BridgeSupportedAsset> {
        let response = self
            .client
            .get(format!("{POLYMARKET_BRIDGE_BASE_URL}/supported-assets"))
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BRIDGE_SUPPORTED_ASSETS_REQUEST_FAILED",
                    format!("failed to query Polymarket Bridge supported assets: {error}"),
                )
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BRIDGE_SUPPORTED_ASSETS_STATUS_FAILED",
                    format!("Polymarket Bridge supported-assets returned error status: {error}"),
                )
            })?
            .json::<BridgeSupportedAssetsResponse>()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BRIDGE_SUPPORTED_ASSETS_DECODE_FAILED",
                    format!("failed to decode Polymarket Bridge supported assets: {error}"),
                )
            })?;

        let expected_address = token.address.to_ascii_lowercase();
        response
            .supported_assets
            .into_iter()
            .find(|asset| {
                asset.chain_id == POLYGON_CHAIN_ID.to_string()
                    && asset.token.address.eq_ignore_ascii_case(&expected_address)
            })
            .ok_or_else(|| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BRIDGE_TOKEN_UNSUPPORTED",
                    format!(
                        "Polymarket Bridge does not currently list {} on Polygon",
                        token.symbol
                    ),
                )
            })
    }

    async fn fetch_bridge_deposit_address(&self, polymarket_wallet_address: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{POLYMARKET_BRIDGE_BASE_URL}/deposit"))
            .json(&serde_json::json!({ "address": polymarket_wallet_address }))
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BRIDGE_DEPOSIT_REQUEST_FAILED",
                    format!("failed to create Polymarket Bridge deposit address: {error}"),
                )
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BRIDGE_DEPOSIT_STATUS_FAILED",
                    format!("Polymarket Bridge deposit endpoint returned error status: {error}"),
                )
            })?
            .json::<BridgeDepositResponse>()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYMARKET_BRIDGE_DEPOSIT_DECODE_FAILED",
                    format!("failed to decode Polymarket Bridge deposit address: {error}"),
                )
            })?;

        response.address.evm.ok_or_else(|| {
            AppError::dependency_unavailable(
                "POLYMARKET_BRIDGE_DEPOSIT_EVM_ADDRESS_MISSING",
                "Polymarket Bridge deposit response did not include an EVM address",
            )
        })
    }

    async fn fetch_erc20_balance(
        &self,
        token_address: &str,
        wallet_address: &str,
        decimals: u32,
    ) -> Result<Decimal> {
        let token_address = normalize_evm_address(
            "token_address",
            token_address,
            "POLYGON_TOKEN_ADDRESS_INVALID",
        )?;
        let wallet_address = normalize_evm_address(
            "wallet_address",
            wallet_address,
            "POLYGON_WALLET_ADDRESS_INVALID",
        )?;
        let wallet_arg = wallet_address
            .trim_start_matches("0x")
            .trim_start_matches("0X");
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [
                {
                    "to": token_address,
                    "data": format!("0x{ERC20_BALANCE_OF_SELECTOR}{wallet_arg:0>64}")
                },
                "latest"
            ]
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYGON_RPC_REQUEST_FAILED",
                    format!("failed to query Polygon RPC: {error}"),
                )
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYGON_RPC_STATUS_FAILED",
                    format!("Polygon RPC returned error status: {error}"),
                )
            })?;

        let payload = response.json::<JsonRpcResponse>().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYGON_RPC_DECODE_FAILED",
                format!("failed to decode Polygon RPC response: {error}"),
            )
        })?;
        if let Some(error) = payload.error {
            return Err(AppError::dependency_unavailable(
                "POLYGON_RPC_ERROR",
                format!("Polygon RPC error {}: {}", error.code, error.message),
            ));
        }
        let raw_hex = payload.result.ok_or_else(|| {
            AppError::dependency_unavailable(
                "POLYGON_RPC_MISSING_RESULT",
                "Polygon RPC response did not include a result",
            )
        })?;

        erc20_hex_units_to_decimal(&raw_hex, decimals)
    }

    async fn ensure_safe_owner_threshold_one(
        &self,
        safe_address: &str,
        expected_owner: AlloyAddress,
    ) -> Result<()> {
        let owners = self.safe_owners(safe_address).await?;
        if !owners.iter().any(|owner| *owner == expected_owner) {
            return Err(AppError::invalid_input(
                "POLYMARKET_SAFE_OWNER_MISMATCH",
                format!(
                    "configured private key address {expected_owner:#x} is not an owner of proxy wallet {safe_address}"
                ),
            ));
        }
        let threshold = self.safe_threshold(safe_address).await?;
        if threshold != AlloyU256::from(1_u8) {
            return Err(AppError::invalid_input(
                "POLYMARKET_SAFE_THRESHOLD_UNSUPPORTED",
                format!("automatic merge currently supports Safe threshold=1 only, got {threshold}"),
            ));
        }
        Ok(())
    }

    async fn safe_owners(&self, safe_address: &str) -> Result<Vec<AlloyAddress>> {
        let bytes = self.eth_call(safe_address, &SAFE_GET_OWNERS_SELECTOR).await?;
        decode_address_array_result(&bytes, "POLYMARKET_SAFE_OWNERS_INVALID")
    }

    async fn safe_threshold(&self, safe_address: &str) -> Result<AlloyU256> {
        let bytes = self
            .eth_call(safe_address, &SAFE_GET_THRESHOLD_SELECTOR)
            .await?;
        decode_u256_result(&bytes, "POLYMARKET_SAFE_THRESHOLD_INVALID")
    }

    async fn safe_nonce(&self, safe_address: &str) -> Result<AlloyU256> {
        let bytes = self.eth_call(safe_address, &SAFE_NONCE_SELECTOR).await?;
        decode_u256_result(&bytes, "POLYMARKET_SAFE_NONCE_INVALID")
    }

    async fn eth_call(&self, to_address: &str, data: &[u8]) -> Result<Vec<u8>> {
        let to_address = normalize_evm_address(
            "to_address",
            to_address,
            "POLYGON_ETH_CALL_TO_ADDRESS_INVALID",
        )?;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [
                {
                    "to": to_address,
                    "data": format!("0x{}", bytes_to_lower_hex(data))
                },
                "latest"
            ]
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYGON_RPC_REQUEST_FAILED",
                    format!("failed to query Polygon RPC: {error}"),
                )
            })?
            .error_for_status()
            .map_err(|error| {
                AppError::dependency_unavailable(
                    "POLYGON_RPC_STATUS_FAILED",
                    format!("Polygon RPC returned error status: {error}"),
                )
            })?;
        let payload = response.json::<JsonRpcResponse>().await.map_err(|error| {
            AppError::dependency_unavailable(
                "POLYGON_RPC_DECODE_FAILED",
                format!("failed to decode Polygon RPC response: {error}"),
            )
        })?;
        if let Some(error) = payload.error {
            return Err(AppError::dependency_unavailable(
                "POLYGON_RPC_ERROR",
                format!("Polygon RPC error {}: {}", error.code, error.message),
            ));
        }
        let raw_hex = payload.result.ok_or_else(|| {
            AppError::dependency_unavailable(
                "POLYGON_RPC_MISSING_RESULT",
                "Polygon RPC response did not include a result",
            )
        })?;
        hex_to_bytes(&raw_hex)
    }
}

fn funding_token_by_id(token_id: &str) -> Result<PolymarketFundingToken> {
    POLYGON_FUNDING_TOKENS
        .iter()
        .copied()
        .find(|token| token.id == token_id)
        .ok_or_else(|| {
            AppError::invalid_input(
                "POLYGON_FUNDING_TOKEN_UNSUPPORTED",
                format!("unsupported Polygon funding token: {token_id}"),
            )
        })
}

fn funding_private_key_signer(private_key: &str, chain_id: u64) -> Result<PrivateKeySigner> {
    let private_key = normalize_optional_text(Some(private_key.to_string())).ok_or_else(|| {
        AppError::invalid_input(
            "POLYMARKET_PRIVATE_KEY_REQUIRED",
            "polymarket private_key must be configured before funding transfer",
        )
    })?;
    PrivateKeySigner::from_str(&private_key)
        .map_err(|error| {
            AppError::invalid_input(
                "POLYMARKET_PRIVATE_KEY_INVALID",
                format!("invalid polymarket private_key: {error}"),
            )
        })
        .map(|signer| signer.with_chain_id(Some(chain_id)))
}

fn parse_alloy_address(name: &str, value: &str, code: &'static str) -> Result<AlloyAddress> {
    let normalized = normalize_evm_address(name, value, code)?;
    AlloyAddress::from_str(&normalized).map_err(|error| {
        AppError::invalid_input(
            code,
            format!("{name} is not a valid EVM address: {error}"),
        )
    })
}

fn decimal_amount_to_units(amount: Decimal, decimals: u8) -> Result<AlloyU256> {
    if amount <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "POLYGON_FUNDING_AMOUNT_INVALID",
            "funding amount must be greater than zero",
        ));
    }

    let normalized = amount.normalize().to_string();
    let (whole, fractional) = normalized
        .split_once('.')
        .map_or((normalized.as_str(), ""), |(whole, fractional)| {
            (whole, fractional)
        });
    if !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fractional.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AppError::invalid_input(
            "POLYGON_FUNDING_AMOUNT_INVALID",
            "funding amount must be a positive decimal number",
        ));
    }
    if fractional.len() > usize::from(decimals) {
        return Err(AppError::invalid_input(
            "POLYGON_FUNDING_AMOUNT_PRECISION_INVALID",
            format!("funding amount cannot have more than {decimals} decimal places"),
        ));
    }

    let mut digits = String::with_capacity(whole.len() + usize::from(decimals));
    digits.push_str(whole);
    digits.push_str(fractional);
    digits.extend(std::iter::repeat_n(
        '0',
        usize::from(decimals).saturating_sub(fractional.len()),
    ));
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        return Err(AppError::invalid_input(
            "POLYGON_FUNDING_AMOUNT_INVALID",
            "funding amount is below token precision",
        ));
    }
    let units = trimmed.parse::<u128>().map_err(|error| {
        AppError::invalid_input(
            "POLYGON_FUNDING_AMOUNT_TOO_LARGE",
            format!("funding amount is too large: {error}"),
        )
    })?;

    Ok(AlloyU256::from(units))
}

fn build_erc20_transfer_calldata(recipient: AlloyAddress, amount_units: AlloyU256) -> Vec<u8> {
    let mut data = Vec::with_capacity(68);
    data.extend_from_slice(&ERC20_TRANSFER_SELECTOR);
    data.extend_from_slice(&[0_u8; 12]);
    data.extend_from_slice(recipient.as_slice());
    data.extend_from_slice(&amount_units.to_be_bytes::<32>());
    data
}

fn build_ctf_merge_positions_calldata(
    collateral_token: AlloyAddress,
    condition_id: [u8; 32],
    amount_units: AlloyU256,
) -> Vec<u8> {
    let mut data = selector("mergePositions(address,bytes32,bytes32,uint256[],uint256)");
    push_abi_address(&mut data, collateral_token);
    push_abi_word(&mut data, [0_u8; 32]);
    push_abi_word(&mut data, condition_id);
    push_abi_u256(&mut data, AlloyU256::from(160_u16));
    push_abi_u256(&mut data, amount_units);
    push_abi_u256(&mut data, AlloyU256::from(2_u8));
    push_abi_u256(&mut data, AlloyU256::from(1_u8));
    push_abi_u256(&mut data, AlloyU256::from(2_u8));
    data
}

#[allow(clippy::too_many_arguments)]
fn build_safe_get_transaction_hash_calldata(
    to: AlloyAddress,
    value: AlloyU256,
    inner_data: &[u8],
    operation: u8,
    safe_tx_gas: AlloyU256,
    base_gas: AlloyU256,
    gas_price: AlloyU256,
    gas_token: AlloyAddress,
    refund_receiver: AlloyAddress,
    nonce: AlloyU256,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&SAFE_GET_TRANSACTION_HASH_SELECTOR);
    push_safe_tx_args(
        &mut data,
        to,
        value,
        inner_data,
        operation,
        safe_tx_gas,
        base_gas,
        gas_price,
        gas_token,
        refund_receiver,
        nonce,
    );
    data
}

#[allow(clippy::too_many_arguments)]
fn build_safe_exec_transaction_calldata(
    to: AlloyAddress,
    value: AlloyU256,
    inner_data: &[u8],
    operation: u8,
    safe_tx_gas: AlloyU256,
    base_gas: AlloyU256,
    gas_price: AlloyU256,
    gas_token: AlloyAddress,
    refund_receiver: AlloyAddress,
    signatures: &[u8],
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&SAFE_EXEC_TRANSACTION_SELECTOR);
    push_abi_address(&mut data, to);
    push_abi_u256(&mut data, value);
    push_abi_u256(&mut data, AlloyU256::from(320_u16));
    push_abi_u256(&mut data, AlloyU256::from(operation));
    push_abi_u256(&mut data, safe_tx_gas);
    push_abi_u256(&mut data, base_gas);
    push_abi_u256(&mut data, gas_price);
    push_abi_address(&mut data, gas_token);
    push_abi_address(&mut data, refund_receiver);
    push_abi_u256(&mut data, AlloyU256::from(320_u16 + padded_len(inner_data)));
    push_abi_bytes(&mut data, inner_data);
    push_abi_bytes(&mut data, signatures);
    data
}

#[allow(clippy::too_many_arguments)]
fn push_safe_tx_args(
    data: &mut Vec<u8>,
    to: AlloyAddress,
    value: AlloyU256,
    inner_data: &[u8],
    operation: u8,
    safe_tx_gas: AlloyU256,
    base_gas: AlloyU256,
    gas_price: AlloyU256,
    gas_token: AlloyAddress,
    refund_receiver: AlloyAddress,
    nonce: AlloyU256,
) {
    push_abi_address(data, to);
    push_abi_u256(data, value);
    push_abi_u256(data, AlloyU256::from(320_u16));
    push_abi_u256(data, AlloyU256::from(operation));
    push_abi_u256(data, safe_tx_gas);
    push_abi_u256(data, base_gas);
    push_abi_u256(data, gas_price);
    push_abi_address(data, gas_token);
    push_abi_address(data, refund_receiver);
    push_abi_u256(data, nonce);
    push_abi_bytes(data, inner_data);
}

fn selector(signature: &str) -> Vec<u8> {
    keccak256(signature.as_bytes())[..4].to_vec()
}

fn push_abi_address(data: &mut Vec<u8>, address: AlloyAddress) {
    data.extend_from_slice(&[0_u8; 12]);
    data.extend_from_slice(address.as_slice());
}

fn push_abi_u256(data: &mut Vec<u8>, value: AlloyU256) {
    data.extend_from_slice(&value.to_be_bytes::<32>());
}

fn push_abi_word(data: &mut Vec<u8>, word: [u8; 32]) {
    data.extend_from_slice(&word);
}

fn push_abi_bytes(data: &mut Vec<u8>, bytes: &[u8]) {
    push_abi_u256(data, AlloyU256::from(bytes.len()));
    data.extend_from_slice(bytes);
    let padding = (32 - (bytes.len() % 32)) % 32;
    data.extend(std::iter::repeat_n(0_u8, padding));
}

fn padded_len(bytes: &[u8]) -> u16 {
    let len = bytes.len() + ((32 - (bytes.len() % 32)) % 32);
    u16::try_from(32 + len).unwrap_or(u16::MAX)
}

fn decode_u256_result(bytes: &[u8], code: &'static str) -> Result<AlloyU256> {
    if bytes.len() < 32 {
        return Err(AppError::dependency_unavailable(
            code,
            "Polygon RPC returned fewer than 32 bytes",
        ));
    }
    Ok(AlloyU256::from_be_slice(&bytes[..32]))
}

fn decode_bytes32_result(bytes: &[u8], code: &'static str) -> Result<[u8; 32]> {
    if bytes.len() < 32 {
        return Err(AppError::dependency_unavailable(
            code,
            "Polygon RPC returned fewer than 32 bytes",
        ));
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes[..32]);
    Ok(out)
}

fn decode_address_array_result(bytes: &[u8], code: &'static str) -> Result<Vec<AlloyAddress>> {
    if bytes.len() < 64 {
        return Err(AppError::dependency_unavailable(
            code,
            "Polygon RPC returned an invalid dynamic address array",
        ));
    }
    let offset = usize::try_from(AlloyU256::from_be_slice(&bytes[..32])).map_err(|error| {
        AppError::dependency_unavailable(code, format!("invalid owner array offset: {error}"))
    })?;
    if bytes.len() < offset + 32 {
        return Err(AppError::dependency_unavailable(
            code,
            "Polygon RPC returned a truncated owner array",
        ));
    }
    let len =
        usize::try_from(AlloyU256::from_be_slice(&bytes[offset..offset + 32])).map_err(|error| {
            AppError::dependency_unavailable(code, format!("invalid owner array length: {error}"))
        })?;
    let start = offset + 32;
    let end = start + len.saturating_mul(32);
    if bytes.len() < end {
        return Err(AppError::dependency_unavailable(
            code,
            "Polygon RPC returned a truncated owner array body",
        ));
    }
    Ok((0..len)
        .map(|index| {
            let word = &bytes[start + index * 32..start + (index + 1) * 32];
            AlloyAddress::from_slice(&word[12..32])
        })
        .collect())
}

fn parse_bytes32(name: &str, value: &str, code: &'static str) -> Result<[u8; 32]> {
    let bytes = hex_to_bytes(value).map_err(|error| {
        AppError::invalid_input(code, format!("{name} must be a 0x-prefixed bytes32: {error}"))
    })?;
    if bytes.len() != 32 {
        return Err(AppError::invalid_input(
            code,
            format!("{name} must contain exactly 32 bytes"),
        ));
    }
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_to_bytes(value: &str) -> Result<Vec<u8>> {
    let raw = value
        .trim()
        .strip_prefix("0x")
        .or_else(|| value.trim().strip_prefix("0X"))
        .ok_or_else(|| {
            AppError::invalid_input("HEX_VALUE_INVALID", "hex value must be 0x-prefixed")
        })?;
    if raw.len() % 2 != 0 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_input(
            "HEX_VALUE_INVALID",
            "hex value must contain an even number of hex characters",
        ));
    }
    let mut bytes = Vec::with_capacity(raw.len() / 2);
    for index in (0..raw.len()).step_by(2) {
        let byte = u8::from_str_radix(&raw[index..index + 2], 16).map_err(|error| {
            AppError::invalid_input("HEX_VALUE_INVALID", format!("invalid hex byte: {error}"))
        })?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn normalize_evm_address(name: &str, value: &str, code: &'static str) -> Result<String> {
    let address = value.trim();
    let Some(raw) = address
        .strip_prefix("0x")
        .or_else(|| address.strip_prefix("0X"))
    else {
        return Err(AppError::invalid_input(
            code,
            format!("{name} must be a 0x-prefixed address"),
        ));
    };
    if raw.len() != 40 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_input(
            code,
            format!("{name} must contain exactly 40 hex characters"),
        ));
    }
    Ok(format!("0x{raw}"))
}

fn normalize_transaction_hash(value: &str) -> Result<String> {
    let value = value.trim();
    let Some(raw) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    else {
        return Err(AppError::invalid_input(
            "POLYGON_TRANSACTION_HASH_INVALID",
            "transaction hash must be 0x-prefixed",
        ));
    };
    if raw.len() != 64 || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AppError::invalid_input(
            "POLYGON_TRANSACTION_HASH_INVALID",
            "transaction hash must contain exactly 64 hex characters",
        ));
    }
    Ok(format!("0x{}", raw.to_ascii_lowercase()))
}

fn erc20_hex_units_to_decimal(raw_hex: &str, decimals: u32) -> Result<Decimal> {
    let raw = raw_hex.trim();
    let raw = raw
        .strip_prefix("0x")
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);
    if raw.is_empty() {
        return Ok(Decimal::ZERO);
    }
    let units = u128::from_str_radix(raw, 16).map_err(|error| {
        AppError::dependency_unavailable(
            "POLYGON_RPC_BALANCE_PARSE_FAILED",
            format!("failed to parse ERC20 balance hex value: {error}"),
        )
    })?;
    if units > i128::MAX as u128 {
        return Err(AppError::dependency_unavailable(
            "POLYGON_RPC_BALANCE_TOO_LARGE",
            "ERC20 balance is too large to fit decimal storage",
        ));
    }
    Ok(Decimal::from_i128_with_scale(units as i128, decimals).round_dp(4))
}
