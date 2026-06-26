const POLYGON_RPC_TIMEOUT: Duration = Duration::from_secs(15);
const POLYGON_CHAIN_ID: u64 = 137;
const POLYMARKET_BRIDGE_BASE_URL: &str = "https://bridge.polymarket.com";
const POLYMARKET_PUSD_CONTRACT_ADDRESS: &str = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB";
const ERC20_BALANCE_OF_SELECTOR: &str = "70a08231";
const ERC20_TRANSFER_SELECTOR: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];
const ERC20_DECIMALS: u32 = 6;

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
        self.fetch_erc20_balance(POLYMARKET_PUSD_CONTRACT_ADDRESS, wallet_address)
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

    async fn fetch_erc20_balance(&self, token_address: &str, wallet_address: &str) -> Result<Decimal> {
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

        erc20_hex_units_to_decimal(&raw_hex, ERC20_DECIMALS)
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
