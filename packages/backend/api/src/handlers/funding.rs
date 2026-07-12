const FUNDING_IDEMPOTENCY_SCOPE: &str = "funding.transfer";
const FUNDING_MAX_TRANSFER_AMOUNT_UNITS: u32 = 10_000;

async fn read_funding_status(
    Extension(auth): Extension<AuthContext>,
    State(state): State<AppState>,
) -> std::result::Result<Json<ApiResponse<FundingStatusData>>, HttpError> {
    let trace_id = new_trace_id();
    let (source_address, polymarket_wallet_address, configuration_error) =
        funding_configuration(&state);
    let (tokens, balance_error) = funding_tokens_to_contract(&state, source_address.as_deref()).await;

    let enabled = configuration_error.is_none()
        && source_address.is_some()
        && polymarket_wallet_address.is_some()
        && state.settings.polymarket.chain_id == 137;

    Ok(Json(ApiResponse::new(
        FundingStatusData {
            enabled,
            source_address,
            polymarket_wallet_address,
            chain_id: state.settings.polymarket.chain_id,
            max_transfer_amount: funding_max_transfer_amount(),
            tokens,
            configuration_error,
            balance_error,
        },
        auth.request_id,
        trace_id,
    )))
}

async fn submit_funding_transfer(
    Extension(auth): Extension<AuthContext>,
    Extension(idempotency_key): Extension<IdempotencyKey>,
    State(state): State<AppState>,
    Json(payload): Json<FundingTransferRequest>,
) -> std::result::Result<Json<ApiResponse<FundingTransferData>>, HttpError> {
    auth.ensure_scope(
        StepUpScope::FundingTransfer,
        time::OffsetDateTime::now_utc(),
    )
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), new_trace_id()))?;

    let trace_id = new_trace_id();
    validate_funding_transfer_request(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let polymarket_wallet_address = configured_polymarket_wallet_address(&state)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let request_hash = hash_json(&payload)
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;
    let idempotency_request = IdempotencyRequest {
        scope: FUNDING_IDEMPOTENCY_SCOPE.to_string(),
        idempotency_key: idempotency_key.0,
        request_hash,
        request_id: auth.request_id.clone(),
        actor_user_id: Some(auth.user_id.clone()),
        actor_session_id: Some(auth.session_id.clone()),
        resource_type: Some("polymarket_wallet".to_string()),
        resource_id: Some(polymarket_wallet_address.clone()),
    };

    match state
        .idempotency_store
        .begin(&idempotency_request)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?
    {
        IdempotencyBegin::Replay(response_json) => {
            let mut replayed: FundingTransferData =
                serde_json::from_str(&response_json).map_err(|error| {
                    HttpError::with_meta(
                        AppError::internal(
                            "FUNDING_TRANSFER_REPLAY_DESERIALIZE_FAILED",
                            format!(
                                "failed to deserialize replayed funding transfer response: {error}"
                            ),
                        ),
                        auth.request_id.clone(),
                        trace_id.clone(),
                    )
                })?;
            replayed.replayed = true;

            append_funding_audit(
                &state,
                &auth,
                &trace_id,
                &replayed.tx_hash,
                payload.operator_note.as_deref().unwrap_or("idempotent funding replay"),
                AuditResult::Succeeded,
            )
            .await
            .map_err(|error| {
                HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone())
            })?;

            return Ok(Json(ApiResponse::new(replayed, auth.request_id, trace_id)));
        }
        IdempotencyBegin::Started => {}
    }

    let private_key = match required_funding_private_key(&state) {
        Ok(private_key) => private_key,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };
    let connector = match PolymarketChainConnector::new(&state.settings.polymarket.polygon_rpc_url)
    {
        Ok(connector) => connector,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let receipt = match connector
        .submit_funding_transfer(
            &private_key,
            state.settings.polymarket.chain_id,
            ConnectorFundingTransferRequest {
                polymarket_wallet_address,
                token_id: payload.token_id,
                amount: payload.amount,
            },
        )
        .await
    {
        Ok(receipt) => receipt,
        Err(error) => {
            state
                .idempotency_store
                .fail(&idempotency_request, error.code())
                .await
                .map_err(|fail_error| {
                    HttpError::with_meta(fail_error, auth.request_id.clone(), trace_id.clone())
                })?;
            return Err(HttpError::with_meta(
                error,
                auth.request_id.clone(),
                trace_id,
            ));
        }
    };

    let response_data = funding_transfer_to_contract(receipt, state.settings.polymarket.chain_id);
    let response_json = serde_json::to_string(&response_data).map_err(|error| {
        HttpError::with_meta(
            AppError::internal(
                "FUNDING_TRANSFER_RESPONSE_SERIALIZE_FAILED",
                format!("failed to serialize funding transfer response: {error}"),
            ),
            auth.request_id.clone(),
            trace_id.clone(),
        )
    })?;

    state
        .idempotency_store
        .complete(&idempotency_request, &response_json)
        .await
        .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    append_funding_audit(
        &state,
        &auth,
        &trace_id,
        &response_data.tx_hash,
        payload.operator_note.as_deref().unwrap_or("funding transfer"),
        AuditResult::Succeeded,
    )
    .await
    .map_err(|error| HttpError::with_meta(error, auth.request_id.clone(), trace_id.clone()))?;

    Ok(Json(ApiResponse::new(
        response_data,
        auth.request_id,
        trace_id,
    )))
}

fn funding_max_transfer_amount() -> Decimal {
    Decimal::from(FUNDING_MAX_TRANSFER_AMOUNT_UNITS)
}

async fn append_funding_audit(
    state: &AppState,
    auth: &AuthContext,
    trace_id: &str,
    tx_hash: &str,
    reason: &str,
    result: AuditResult,
) -> polyedge_domain::Result<()> {
    state
        .audit_log_sink
        .append(AuditLogEntry {
            occurred_at: time::OffsetDateTime::now_utc(),
            request_id: auth.request_id.clone(),
            trace_id: trace_id.to_string(),
            actor: AuthenticatedActor {
                user_id: auth.user_id.clone(),
                session_id: auth.session_id.clone(),
                roles: auth.roles.clone(),
                request_id: auth.request_id.clone(),
                ip: auth.ip.clone(),
                user_agent: auth.user_agent.clone(),
            },
            action: "funding_transfer".to_string(),
            resource_type: "polygon_transaction".to_string(),
            resource_id: tx_hash.to_string(),
            reason: reason.to_string(),
            result,
            error_code: None,
        })
        .await
}

async fn funding_tokens_to_contract(
    state: &AppState,
    source_address: Option<&str>,
) -> (Vec<FundingTokenData>, Option<String>) {
    let mut tokens: Vec<FundingTokenData> = PolymarketChainConnector::polygon_funding_tokens()
        .iter()
        .map(|token| FundingTokenData {
            id: token.id.to_string(),
            symbol: token.symbol.to_string(),
            name: token.name.to_string(),
            address: token.address.to_string(),
            decimals: token.decimals,
            min_transfer_amount: token.min_checkout_usd,
            balance: None,
        })
        .collect();

    let Some(source_address) = source_address else {
        return (tokens, None);
    };
    if state.settings.polymarket.chain_id != 137
        || state.settings.polymarket.polygon_rpc_url.trim().is_empty()
    {
        return (tokens, None);
    }

    let connector = match PolymarketChainConnector::new(&state.settings.polymarket.polygon_rpc_url)
    {
        Ok(connector) => connector,
        Err(error) => return (tokens, Some(error.message().to_string())),
    };
    let mut balance_error = None;
    for token in &mut tokens {
        match connector
            .fetch_funding_token_balance(&token.id, source_address)
            .await
        {
            Ok(balance) => token.balance = Some(balance),
            Err(error) => {
                balance_error = Some(error.message().to_string());
                break;
            }
        }
    }

    (tokens, balance_error)
}

fn funding_transfer_to_contract(
    receipt: polyedge_connectors::PolymarketFundingTransferReceipt,
    chain_id: u64,
) -> FundingTransferData {
    FundingTransferData {
        tx_hash: receipt.tx_hash,
        source_address: receipt.source_address,
        polymarket_wallet_address: receipt.polymarket_wallet_address,
        bridge_deposit_address: receipt.bridge_deposit_address,
        token_id: receipt.token.id.to_string(),
        token_symbol: receipt.token.symbol.to_string(),
        token_address: receipt.token.address.to_string(),
        amount: receipt.amount,
        amount_units: receipt.amount_units,
        chain_id,
        replayed: false,
    }
}

fn validate_funding_transfer_request(payload: &FundingTransferRequest) -> polyedge_domain::Result<()> {
    if let Some(note) = payload.operator_note.as_deref() {
        let note = note.trim();
        if note.len() > 500 || note.contains('\r') || note.contains('\n') {
            return Err(AppError::invalid_input(
                "FUNDING_OPERATOR_NOTE_INVALID",
                "funding operator_note must be a single line of at most 500 bytes",
            ));
        }
    }
    if !payload.confirmed {
        return Err(AppError::invalid_input(
            "FUNDING_TRANSFER_CONFIRMATION_REQUIRED",
            "funding transfer requires explicit confirmation",
        ));
    }
    if payload.amount <= Decimal::ZERO {
        return Err(AppError::invalid_input(
            "FUNDING_TRANSFER_AMOUNT_INVALID",
            "funding transfer amount must be greater than zero",
        ));
    }
    if payload.amount > funding_max_transfer_amount() {
        return Err(AppError::invalid_input(
            "FUNDING_TRANSFER_AMOUNT_TOO_LARGE",
            format!(
                "funding transfer amount cannot exceed {}",
                funding_max_transfer_amount()
            ),
        ));
    }
    if !PolymarketChainConnector::polygon_funding_tokens()
        .iter()
        .any(|token| token.id == payload.token_id)
    {
        return Err(AppError::invalid_input(
            "FUNDING_TRANSFER_TOKEN_UNSUPPORTED",
            format!("unsupported funding token: {}", payload.token_id),
        ));
    }

    Ok(())
}

fn funding_configuration(state: &AppState) -> (Option<String>, Option<String>, Option<String>) {
    let mut configuration_error = None;

    if state.settings.polymarket.chain_id != 137 {
        configuration_error = Some(format!(
            "funding transfers require Polygon chain_id=137, got {}",
            state.settings.polymarket.chain_id
        ));
    }

    let source_address = match state.settings.polymarket.private_key.as_deref() {
        Some(private_key) if !private_key.trim().is_empty() => {
            match PolymarketChainConnector::funding_source_address(
                private_key,
                state.settings.polymarket.chain_id,
            ) {
                Ok(address) => Some(address),
                Err(error) => {
                    configuration_error.get_or_insert_with(|| error.message().to_string());
                    None
                }
            }
        }
        _ => {
            configuration_error
                .get_or_insert_with(|| "polymarket private_key is not configured".to_string());
            None
        }
    };

    let polymarket_wallet_address = match configured_polymarket_wallet_address(state) {
        Ok(address) => Some(address),
        Err(error) => {
            configuration_error.get_or_insert_with(|| error.message().to_string());
            None
        }
    };

    if state.settings.polymarket.polygon_rpc_url.trim().is_empty() {
        configuration_error.get_or_insert_with(|| "polygon rpc url is not configured".to_string());
    }

    (
        source_address,
        polymarket_wallet_address,
        configuration_error,
    )
}

fn configured_polymarket_wallet_address(state: &AppState) -> polyedge_domain::Result<String> {
    let raw = state
        .settings
        .polymarket
        .funder
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&state.settings.polymarket.account_id);
    if raw.trim().is_empty() {
        return Err(AppError::invalid_input(
            "POLYMARKET_FUNDING_WALLET_ADDRESS_REQUIRED",
            "polymarket funder or account_id must be configured before funding transfer",
        ));
    }

    PolymarketChainConnector::normalize_funding_wallet_address(raw)
}

fn required_funding_private_key(state: &AppState) -> polyedge_domain::Result<String> {
    state
        .settings
        .polymarket
        .private_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(std::borrow::ToOwned::to_owned)
        .ok_or_else(|| {
            AppError::invalid_input(
                "POLYMARKET_PRIVATE_KEY_REQUIRED",
                "polymarket private_key must be configured before funding transfer",
            )
        })
}
