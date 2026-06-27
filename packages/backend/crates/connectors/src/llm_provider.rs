use polyedge_domain::{AppError, Result};

static LLM_PROVIDER_REQUEST_SEMAPHORE: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(1);

pub(crate) async fn acquire_llm_provider_request_permit()
-> Result<tokio::sync::SemaphorePermit<'static>> {
    LLM_PROVIDER_REQUEST_SEMAPHORE
        .acquire()
        .await
        .map_err(|error| {
            AppError::internal(
                "LLM_PROVIDER_SEMAPHORE_CLOSED",
                format!("LLM provider request semaphore closed: {error}"),
            )
        })
}
