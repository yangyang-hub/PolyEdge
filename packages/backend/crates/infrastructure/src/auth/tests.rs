#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestHeader<'a> {
        alg: &'a str,
        kid: &'a str,
        typ: &'a str,
    }

    #[derive(Serialize)]
    struct TestClaims {
        iss: String,
        aud: String,
        sub: String,
        iat: i64,
        nbf: i64,
        exp: i64,
        jti: String,
        session_id: String,
        roles: Vec<UserRole>,
        auth_time: i64,
        request_id: String,
        step_up_verified: bool,
        step_up_scope: Vec<StepUpScope>,
        step_up_until: Option<i64>,
    }

    fn issue_token(
        signing_key: &SigningKey,
        kid: &str,
        request_id: &str,
        include_scope: bool,
    ) -> String {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let header = serde_json::to_vec(&TestHeader {
            alg: "EdDSA",
            kid,
            typ: "JWT",
        })
        .expect("serialize header");
        let claims = serde_json::to_vec(&TestClaims {
            iss: "polyedge-nextjs".to_string(),
            aud: "polyedge-rust-api".to_string(),
            sub: "usr_123".to_string(),
            iat: now,
            nbf: now,
            exp: now + 30,
            jti: "jit_123".to_string(),
            session_id: "sess_123".to_string(),
            roles: vec![UserRole::RiskAdmin],
            auth_time: now - 60,
            request_id: request_id.to_string(),
            step_up_verified: include_scope,
            step_up_scope: if include_scope {
                vec![StepUpScope::SystemModeSwitch]
            } else {
                Vec::new()
            },
            step_up_until: if include_scope { Some(now + 60) } else { None },
        })
        .expect("serialize claims");

        let header_b64 = general_purpose::URL_SAFE_NO_PAD.encode(header);
        let claims_b64 = general_purpose::URL_SAFE_NO_PAD.encode(claims);
        let message = format!("{header_b64}.{claims_b64}");
        let signature = signing_key.sign(message.as_bytes());
        let signature_b64 = general_purpose::URL_SAFE_NO_PAD.encode(signature.to_bytes());
        format!("{message}.{signature_b64}")
    }

    #[test]
    fn verifier_accepts_valid_token() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = general_purpose::STANDARD.encode(signing_key.verifying_key().as_bytes());
        let settings = AuthSettings {
            disabled: false,
            allow_insecure_private_deploy: false,
            issuer: "polyedge-nextjs".to_string(),
            audience: "polyedge-rust-api".to_string(),
            clock_skew_secs: 30,
            max_query_ttl_secs: 60,
            max_write_ttl_secs: 30,
            max_step_up_window_secs: 600,
            step_up_code: String::new(),
            revoked_sessions: Vec::new(),
            force_reauth_after: None,
            keys: vec![crate::settings::AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: public_key,
            }],
        };
        let verifier = InternalTokenVerifier::from_settings(&settings).expect("verifier");
        let token = issue_token(&signing_key, "test-key", "req_123", true);

        let auth = verifier
            .authenticate(
                &token,
                "req_123",
                RequestKind::Write,
                Some("127.0.0.1".to_string()),
                Some("test-agent".to_string()),
            )
            .expect("authenticate");

        assert_eq!(auth.user_id, "usr_123");
        assert!(auth.step_up_verified);
    }

    #[test]
    fn verifier_rejects_request_id_mismatch() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = general_purpose::STANDARD.encode(signing_key.verifying_key().as_bytes());
        let settings = AuthSettings {
            disabled: false,
            allow_insecure_private_deploy: false,
            issuer: "polyedge-nextjs".to_string(),
            audience: "polyedge-rust-api".to_string(),
            clock_skew_secs: 30,
            max_query_ttl_secs: 60,
            max_write_ttl_secs: 30,
            max_step_up_window_secs: 600,
            step_up_code: String::new(),
            revoked_sessions: Vec::new(),
            force_reauth_after: None,
            keys: vec![crate::settings::AuthKeySettings {
                kid: "test-key".to_string(),
                public_key_base64: public_key,
            }],
        };
        let verifier = InternalTokenVerifier::from_settings(&settings).expect("verifier");
        let token = issue_token(&signing_key, "test-key", "req_123", false);

        let error = verifier
            .authenticate(&token, "req_other", RequestKind::Read, None, None)
            .expect_err("request id mismatch should fail");

        assert_eq!(error.code(), "AUTH_INVALID_INTERNAL_TOKEN");
    }
}
