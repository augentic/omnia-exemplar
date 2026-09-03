//! # <PACKAGE_NAME>
//!
//! An Omnia WASI HTTP guest. Only the WASI export is `wasm32`-gated: the
//! router is generic over its provider, so `tests/routes.rs` drives the
//! routing table natively under `omnia_test::provider!` doubles.

use omnia_guest::Config;
use omnia_guest::api::http::post;
use omnia_guest::api::{Client, Context};
use serde::{Deserialize, Serialize};

/// The tenant that owns this deployment.
pub const OWNER: &str = "<PACKAGE_NAME>";

#[cfg(target_arch = "wasm32")]
omnia_guest::provider! {
    /// Bare provider backed by the default WASI capability implementations.
    pub struct Provider: Config;
}

/// WASI HTTP export.
#[cfg(target_arch = "wasm32")]
pub struct Http;
#[cfg(target_arch = "wasm32")]
wasip3::http::service::export!(Http);

#[cfg(target_arch = "wasm32")]
impl wasip3::exports::http::handler::Guest for Http {
    async fn handle(
        request: wasip3::http::types::Request,
    ) -> Result<wasip3::http::types::Response, wasip3::http::types::ErrorCode> {
        omnia_guest::api::http::serve(router(Provider), request).await
    }
}

/// Build the HTTP router over one provider-owning [`Client`].
///
/// The bound is the union of every route handler's capability list.
pub fn router<P>(provider: P) -> axum::Router
where
    P: Config + Send + Sync + 'static,
{
    axum::Router::new()
        .route("/greet", post::<GreetRequest, P>())
        .with_state(Client::new(OWNER, provider))
}

#[omnia_guest::handler]
async fn greet<P>(input: GreetRequest, context: Context<'_, P>) -> omnia_guest::Result<GreetReply>
where
    P: Config,
{
    let greeting = Config::get(context.provider, "GREETING").await?;
    Ok(GreetReply {
        message: format!("{greeting}, {}!", input.name),
    })
}

/// Who to greet.
#[derive(Debug, Clone, Deserialize)]
pub struct GreetRequest {
    /// The caller's name.
    pub name: String,
}

/// The greeting.
#[derive(Debug, Clone, Serialize)]
pub struct GreetReply {
    /// The configured greeting addressed to the caller.
    pub message: String,
}
