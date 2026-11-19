//! The stem's handlers as one component, for the suites under `tests/`.

#![cfg(target_arch = "wasm32")]

use omnia_sdk::api::{Router, http, messaging};

fn router() -> Router {
    capability::handlers().into_router()
}

http::export!(router);
messaging::export!(router);
