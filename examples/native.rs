//! Native server entrypoint over `omnia_native::Direct` and `Serve`.
//!
//! Runs the exemplar's HTTP and messaging routers in-process without a
//! WebAssembly runtime:
//!
//! ```shell
//! cargo run --example native
//! ```

cfg_select! {
    not(target_arch = "wasm32") => {
        use omnia_backend::{
            BlobStoreDefault, ConfigDefault, Connect as _, DocStoreDefault, HttpDefault,
            IdentityDefault, KeyValueDefault, MessagingDefault, SqlDefault, Telemetry,
            WebSocketDefault,
        };
        use omnia_native::{Direct, Serve};

        #[derive(Clone, omnia_sdk::Provider)]
        struct Native {
            #[provider(Config)]
            config: Direct<ConfigDefault>,
            #[provider(HttpRequest)]
            http: Direct<HttpDefault>,
            #[provider(Identity)]
            identity: Direct<IdentityDefault>,
            #[provider(Publish)]
            publish: Direct<MessagingDefault>,
            #[provider(StateStore)]
            state: Direct<KeyValueDefault>,
            #[provider(BlobStore)]
            blob: Direct<BlobStoreDefault>,
            #[provider(DocumentStore)]
            docs: Direct<DocStoreDefault>,
            #[provider(TableStore)]
            tables: Direct<SqlDefault>,
            #[provider(Broadcast)]
            broadcast: Direct<WebSocketDefault>,
        }

        #[tokio::main]
        async fn main() -> anyhow::Result<()> {
            let messaging = MessagingDefault::connect().await?;
            let provider = Native {
                config: Direct::new(ConfigDefault::connect().await?),
                http: Direct::new(HttpDefault::connect().await?),
                identity: Direct::new(IdentityDefault::connect().await?),
                publish: Direct::new(messaging.clone()),
                state: Direct::new(KeyValueDefault::connect().await?),
                blob: Direct::new(BlobStoreDefault::connect().await?),
                docs: Direct::new(DocStoreDefault::connect().await?),
                tables: Direct::new(SqlDefault::connect().await?),
                broadcast: Direct::new(WebSocketDefault::connect().await?),
            };
            Serve::new()
                .telemetry(Telemetry::new("guest"))
                .http("0.0.0.0:8080", guest::router(provider.clone()))
                .messaging(messaging, guest::messaging_router(provider).await)
                .run()
                .await
        }
    }
    _ => {
        fn main() {}
    }
}
