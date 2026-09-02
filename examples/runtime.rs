//! Host runtime for the WASM guest.
//!
//! Build the guest for `wasm32-wasip2` and run it with:
//!
//! ```shell
//! cargo build -p guest --target wasm32-wasip2 --release
//! cargo run --example runtime -- run target/wasm32-wasip2/release/guest.wasm
//! ```

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use omnia_wasi_blobstore::{WasiBlobstore, BlobstoreDefault};
        use omnia_wasi_config::{WasiConfig, ConfigDefault};
        use omnia_wasi_docstore::{WasiDocStore, DocStoreDefault};
        use omnia_wasi_http::{WasiHttp, HttpDefault};
        use omnia_wasi_identity::{WasiIdentity, IdentityDefault};
        use omnia_wasi_keyvalue::{WasiKeyValue, KeyValueDefault};
        use omnia_wasi_messaging::{WasiMessaging, MessagingDefault};
        use omnia_wasi_otel::{WasiOtel, OtelDefault};
        use omnia_wasi_sql::{WasiSql, SqlDefault};
        use omnia_wasi_websocket::{WasiWebSocket, WebSocketDefault};

        omnia::runtime!({
            hosts: {
                WasiBlobstore: BlobstoreDefault,
                WasiConfig: ConfigDefault,
                WasiDocStore: DocStoreDefault,
                WasiHttp: HttpDefault,
                WasiIdentity: IdentityDefault,
                WasiKeyValue: KeyValueDefault,
                WasiMessaging: MessagingDefault,
                WasiOtel: OtelDefault,
                WasiSql: SqlDefault,
                WasiWebSocket: WebSocketDefault,
            }
        });
    } else {
        fn main() {}
    }
}
