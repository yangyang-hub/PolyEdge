use polyedge_application::{
    RewardDecisionReplayFixture, RewardStrategyActionListQuery, RewardStrategyDecisionListQuery,
    RewardStrategyRunListQuery, replay_reward_decision_engine,
};
use polyedge_domain::{AppError, Result};
use polyedge_infrastructure::{Runtime, telemetry::init_tracing};
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};
use tracing::info;

#[derive(Debug)]
enum ReplayCommand {
    Audit { run_id: Option<i64> },
    Fixture { path: PathBuf },
    StoredFixture { run_id: i64 },
}

#[derive(Debug, Serialize)]
struct RunAuditReport {
    run_id: i64,
    trace_id: String,
    status: String,
    config_hash: String,
    decision_count: usize,
    eligible_count: usize,
    fair_value_pass_count: usize,
    blocker_counts: BTreeMap<String, usize>,
    ai_action_counts: BTreeMap<String, usize>,
    info_risk_action_counts: BTreeMap<String, usize>,
    action_status_counts: BTreeMap<String, usize>,
    action_type_counts: BTreeMap<String, usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing("polyedge_replay");
    match requested_command()? {
        ReplayCommand::Fixture { path } => replay_fixture(&path),
        ReplayCommand::StoredFixture { run_id } => replay_stored_fixture(run_id).await,
        ReplayCommand::Audit { run_id } => audit_run(run_id).await,
    }
}

async fn replay_stored_fixture(run_id: i64) -> Result<()> {
    let runtime = Runtime::load().await?;
    let state = runtime.app_state();
    let stored = state
        .reward_bot_service
        .get_strategy_replay_fixture(run_id)
        .await?
        .ok_or_else(|| {
            AppError::not_found(
                "REWARD_REPLAY_FIXTURE_NOT_FOUND",
                format!("no replay fixture is stored for strategy run {run_id}"),
            )
        })?;
    let result = replay_reward_decision_engine(&stored.fixture)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|error| {
            AppError::internal(
                "REWARD_REPLAY_REPORT_SERIALIZATION_FAILED",
                error.to_string(),
            )
        })?
    );
    info!(
        run_id,
        sha256 = %stored.sha256,
        fixture_bytes = stored.json_bytes,
        matches_expected = result.comparison.as_ref().map(|value| value.matches),
        "stored rewards decision fixture replay completed"
    );
    Ok(())
}

fn replay_fixture(path: &PathBuf) -> Result<()> {
    let bytes = std::fs::read(path).map_err(|error| {
        AppError::invalid_input(
            "REWARD_REPLAY_FIXTURE_READ_FAILED",
            format!("failed to read replay fixture {}: {error}", path.display()),
        )
    })?;
    let fixture: RewardDecisionReplayFixture = serde_json::from_slice(&bytes).map_err(|error| {
        AppError::invalid_input(
            "REWARD_REPLAY_FIXTURE_INVALID",
            format!(
                "failed to decode replay fixture {}: {error}",
                path.display()
            ),
        )
    })?;
    let result = replay_reward_decision_engine(&fixture)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(|error| {
            AppError::internal(
                "REWARD_REPLAY_REPORT_SERIALIZATION_FAILED",
                error.to_string(),
            )
        })?
    );
    info!(
        fixture = %path.display(),
        plans = result.summary.plan_count,
        eligible = result.summary.eligible_count,
        matches_expected = result.comparison.as_ref().map(|value| value.matches),
        "rewards decision engine fixture replay completed"
    );
    Ok(())
}

async fn audit_run(run_id: Option<i64>) -> Result<()> {
    let runtime = Runtime::load().await?;
    let state = runtime.app_state();
    let run = match run_id {
        Some(run_id) => state.reward_bot_service.get_strategy_run(run_id).await?,
        None => state
            .reward_bot_service
            .list_strategy_runs(&RewardStrategyRunListQuery::new(
                None,
                None,
                Some(1),
                Some(1),
            ))
            .await?
            .items
            .into_iter()
            .next(),
    }
    .ok_or_else(|| {
        AppError::not_found(
            "REWARD_REPLAY_RUN_NOT_FOUND",
            "no rewards strategy run is available for audit replay",
        )
    })?;

    let decisions = state
        .reward_bot_service
        .list_strategy_decisions(
            run.run_id,
            &RewardStrategyDecisionListQuery::new(None, None, Some(1), Some(500)),
        )
        .await?
        .items;
    let actions = state
        .reward_bot_service
        .list_strategy_actions(
            run.run_id,
            &RewardStrategyActionListQuery::new(None, None, Some(1), Some(500)),
        )
        .await?
        .items;

    let report = RunAuditReport {
        run_id: run.run_id,
        trace_id: run.trace_id,
        status: run.status.as_str().to_string(),
        config_hash: run.config_hash,
        decision_count: decisions.len(),
        eligible_count: decisions
            .iter()
            .filter(|decision| decision.eligible)
            .count(),
        fair_value_pass_count: decisions
            .iter()
            .filter(|decision| decision.fair_value_passed == Some(true))
            .count(),
        blocker_counts: count_values(
            decisions
                .iter()
                .flat_map(|decision| decision.blocker_codes.iter().map(String::as_str)),
        ),
        ai_action_counts: count_values(
            decisions
                .iter()
                .filter_map(|decision| decision.ai_action.as_deref()),
        ),
        info_risk_action_counts: count_values(
            decisions
                .iter()
                .filter_map(|decision| decision.info_risk_action.as_deref()),
        ),
        action_status_counts: count_values(actions.iter().map(|action| action.status.as_str())),
        action_type_counts: count_values(actions.iter().map(|action| action.action_type.as_str())),
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| {
            AppError::internal(
                "REWARD_REPLAY_REPORT_SERIALIZATION_FAILED",
                error.to_string(),
            )
        })?
    );
    info!(
        run_id = run.run_id,
        "rewards strategy run audit replay completed"
    );
    Ok(())
}

fn requested_command() -> Result<ReplayCommand> {
    let mut args = std::env::args().skip(1);
    let Some(argument) = args.next() else {
        return Ok(ReplayCommand::Audit { run_id: None });
    };
    let value = args.next().ok_or_else(|| {
        AppError::invalid_input(
            "REWARD_REPLAY_ARGUMENT_VALUE_REQUIRED",
            format!("{argument} requires a value"),
        )
    })?;
    if args.next().is_some() {
        return Err(AppError::invalid_input(
            "REWARD_REPLAY_ARGUMENT_INVALID",
            replay_usage(),
        ));
    }
    match argument.as_str() {
        "--run-id" => value
            .parse::<i64>()
            .map(|run_id| ReplayCommand::Audit {
                run_id: Some(run_id),
            })
            .map_err(|error| {
                AppError::invalid_input("REWARD_REPLAY_RUN_ID_INVALID", error.to_string())
            }),
        "--fixture" => Ok(ReplayCommand::Fixture {
            path: PathBuf::from(value),
        }),
        "--stored-run-id" => value
            .parse::<i64>()
            .map(|run_id| ReplayCommand::StoredFixture { run_id })
            .map_err(|error| {
                AppError::invalid_input("REWARD_REPLAY_RUN_ID_INVALID", error.to_string())
            }),
        _ => Err(AppError::invalid_input(
            "REWARD_REPLAY_ARGUMENT_INVALID",
            replay_usage(),
        )),
    }
}

fn replay_usage() -> &'static str {
    "usage: polyedge-replay [--run-id RUN_ID | --stored-run-id RUN_ID | --fixture FIXTURE.json]"
}

fn count_values<'a>(values: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for value in values.filter(|value| !value.is_empty()) {
        *counts.entry(value.to_string()).or_default() += 1;
    }
    counts
}
