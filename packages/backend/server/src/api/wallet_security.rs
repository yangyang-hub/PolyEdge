async fn create_wallet_import_context(
    State(state): State<AppState>, headers: HeaderMap,
) -> Result<Json<ApiResponse<polyedge_contracts::WalletImportContextData>>> {
    let context = authorize_mutation(&state, &headers, None).await?;
    if context.actor.role == polyedge_domain::UserRole::ReadOnly {
        return Err(ServerError::Forbidden);
    }
    let issued = state.wallet_crypto.create_durable_import_context()?;
    state.store.persist_wallet_import_context(
        context.actor.user_id, issued.context_id, &issued.key_id, issued.expires_at,
        state.wallet_crypto.max_import_contexts(),
    ).await?;
    Ok(response(polyedge_contracts::WalletImportContextData {
        context_id: issued.context_id.to_string(), key_id: issued.key_id,
        algorithm: issued.algorithm.to_string(), aad_version: issued.aad_version.to_string(),
        public_key: polyedge_contracts::WalletImportPublicJwkData {
            kty: issued.public_key.kty.to_string(), use_: issued.public_key.use_.to_string(),
            alg: issued.public_key.alg.to_string(), kid: issued.public_key.kid,
            n: issued.public_key.n, e: issued.public_key.e,
        },
        expires_at: issued.expires_at,
    }, &context))
}
