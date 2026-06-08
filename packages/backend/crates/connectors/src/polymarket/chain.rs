const POLYGON_RPC_TIMEOUT: Duration = Duration::from_secs(15);
const POLYMARKET_PUSD_CONTRACT_ADDRESS: &str = "0xC011a7E12a19f7B1f670d46F03B03f3342E82DFB";
const ERC20_BALANCE_OF_SELECTOR: &str = "70a08231";
const ERC20_DECIMALS: u32 = 6;

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
