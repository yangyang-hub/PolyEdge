async fn runtime_config_entries_for_state(
    state: &AppState,
) -> polyedge_domain::Result<Vec<polyedge_infrastructure::settings::RuntimeConfigEntry>> {
    let values = state.runtime_config_store.load_values().await?;
    let mut settings = (*state.settings).clone();
    settings.apply_runtime_config_values(values)?;
    Ok(settings.runtime_config_entries())
}

fn runtime_config_entry_to_contract(
    entry: polyedge_infrastructure::settings::RuntimeConfigEntry,
) -> RuntimeConfigEntryData {
    RuntimeConfigEntryData {
        key: entry.key,
        section: entry.section,
        field: entry.field,
        label: entry.label,
        env_name: entry.env_name,
        value: entry.value,
        default_value: entry.default_value,
        value_type: entry.value_type.as_str().to_string(),
        options: entry.options,
        restart_required: entry.restart_required,
    }
}
