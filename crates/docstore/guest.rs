//! The stem's handlers as one component, for the suites under `tests/`.

#![cfg(target_arch = "wasm32")]

use omnia_sdk::{Router, http, messaging};

fn router() -> Router {
    docstore::handlers().into_router()
}

http::export!(router);
messaging::export!(router);
