/// Process-local cooldown for failed rewards provider calls. Successful cache
/// writes clear the entry; non-content-filter failures arm it so the next full
/// ticks skip the same condition instead of retrying every minute.

static REWARD_PROVIDER_FAILURE_COOLDOWN: OnceLock<Mutex<HashMap<String, OffsetDateTime>>> =
    OnceLock::new();

fn reward_provider_failure_cooldown_map() -> &'static Mutex<HashMap<String, OffsetDateTime>> {
    REWARD_PROVIDER_FAILURE_COOLDOWN.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reward_provider_failure_cooldown_active(condition_id: &str, now: OffsetDateTime) -> bool {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return false;
    }
    let Ok(mut guard) = reward_provider_failure_cooldown_map().lock() else {
        return false;
    };
    match guard.get(condition_id).copied() {
        Some(until) if until > now => true,
        Some(_) => {
            guard.remove(condition_id);
            false
        }
        None => false,
    }
}

fn mark_reward_provider_failure_cooldown(
    condition_id: &str,
    cooldown_sec: u64,
    now: OffsetDateTime,
) {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() || cooldown_sec == 0 {
        return;
    }
    let Ok(mut guard) = reward_provider_failure_cooldown_map().lock() else {
        return;
    };
    let until = now + TimeDuration::seconds(cooldown_sec.min(i64::MAX as u64) as i64);
    guard.insert(condition_id.to_string(), until);
    if guard.len() > 4_096 {
        guard.retain(|_, expires_at| *expires_at > now);
    }
}

fn clear_reward_provider_failure_cooldown(condition_id: &str) {
    let condition_id = condition_id.trim();
    if condition_id.is_empty() {
        return;
    }
    if let Ok(mut guard) = reward_provider_failure_cooldown_map().lock() {
        guard.remove(condition_id);
    }
}

#[cfg(test)]
mod reward_provider_failure_cooldown_tests {
    use super::*;

    #[test]
    fn failure_cooldown_blocks_until_expiry_and_clears_on_success() {
        let condition_id = "cond_cooldown_test";
        clear_reward_provider_failure_cooldown(condition_id);
        let now = OffsetDateTime::now_utc();
        assert!(!reward_provider_failure_cooldown_active(condition_id, now));

        mark_reward_provider_failure_cooldown(condition_id, 600, now);
        assert!(reward_provider_failure_cooldown_active(condition_id, now));
        assert!(reward_provider_failure_cooldown_active(
            condition_id,
            now + TimeDuration::seconds(599)
        ));
        assert!(!reward_provider_failure_cooldown_active(
            condition_id,
            now + TimeDuration::seconds(601)
        ));

        mark_reward_provider_failure_cooldown(condition_id, 600, now);
        clear_reward_provider_failure_cooldown(condition_id);
        assert!(!reward_provider_failure_cooldown_active(condition_id, now));
    }

    #[test]
    fn zero_cooldown_is_a_no_op() {
        let condition_id = "cond_cooldown_disabled";
        clear_reward_provider_failure_cooldown(condition_id);
        let now = OffsetDateTime::now_utc();
        mark_reward_provider_failure_cooldown(condition_id, 0, now);
        assert!(!reward_provider_failure_cooldown_active(condition_id, now));
    }
}
