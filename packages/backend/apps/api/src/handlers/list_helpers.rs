fn apply_limit<T>(mut items: Vec<T>, limit: Option<u16>) -> Vec<T> {
    let limit = usize::from(
        limit
            .unwrap_or(DEFAULT_CONSOLE_LIST_LIMIT)
            .min(MAX_CONSOLE_LIST_LIMIT),
    );
    items.truncate(limit);
    items
}
