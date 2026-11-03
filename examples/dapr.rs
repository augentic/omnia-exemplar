//! Dapr sidecar entrypoint over `omnia_dapr::Dapr` and `omnia_native::Direct`.
//!
//! Serves `Config`, `StateStore`, and `Publish` through the Dapr HTTP v1.0
//! sidecar, delegates the remaining capabilities to `Direct<*Default>`, and
//! mounts Dapr's `CloudEvents` subscription routes onto the HTTP router:
//!
//! ```shell
//! dapr run --app-id guest --app-port 8080 -- cargo run --example dapr
//! ```

cfg_select! {
    not(target_arch = "wasm32") => {
        use omnia_backend::{
            BlobStoreDefault, Connect as _, DocStoreDefault, HttpDefault, IdentityDefault,
            SqlDefault, Telemetry, WebSocketDefault,
        };
        use omnia_dapr::Dapr;
        use omnia_dapr::axum::subscriptions;
        use omnia_native::{Direct, Serve};

        #[derive(Clone, omnia_sdk::Provider)]
        struct App {
            #[provider(Config, StateStore, Publish)]
            dapr: Dapr<Direct<HttpDefault>>,
            #[provider(HttpRequest)]
            http: Direct<HttpDefault>,
            #[provider(Identity)]
            identity: Direct<IdentityDefault>,
            #[provider(TableStore)]
            tables: Direct<SqlDefault>,
            #[provider(DocumentStore)]
            docs: Direct<DocStoreDefault>,
            #[provider(BlobStore)]
            blob: Direct<BlobStoreDefault>,
            #[provider(Broadcast)]
            broadcast: Direct<WebSocketDefault>,
        }

        #[tokio::main]
        async fn main() -> anyhow::Result<()> {
            let http = Direct::new(HttpDefault::connect().await?);
            // DAPR_HTTP_ENDPOINT, DAPR_STATE_STORE, DAPR_PUBSUB, DAPR_CONFIG_STORE
            let dapr = Dapr::from_env(http.clone())?;
            let provider = App {
                dapr: dapr.clone(),
                http,
                identity: Direct::new(IdentityDefault::connect().await?),
                tables: Direct::new(SqlDefault::connect().await?),
                docs: Direct::new(DocStoreDefault::connect().await?),
                blob: Direct::new(BlobStoreDefault::connect().await?),
                broadcast: Direct::new(WebSocketDefault::connect().await?),
            };
            let messaging = guest::messaging_router(provider.clone()).await;
            let app = guest::router(provider).merge(subscriptions(messaging, &dapr));
            Serve::new().telemetry(Telemetry::new("guest")).http("0.0.0.0:8080", app).run().await
        }
    }
    _ => {
        fn main() {}
    }
}
