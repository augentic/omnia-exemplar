//! Host runtime for the WASM guest.
//!
//! Build the guest for `wasm32-wasip2` and run it with:
//!
//! ```shell
//! cargo build -p guest --target wasm32-wasip2 --release
//! cargo run --example runtime -- run target/wasm32-wasip2/release/guest.wasm
//! ```

cfg_select! {
    not(target_arch = "wasm32") => {
        use omnia_wasi_blobstore::{BlobstoreDefault, WasiBlobstore};
        use omnia_wasi_config::{ConfigDefault, WasiConfig};
        use omnia_wasi_docstore::{DocStoreDefault, WasiDocStore};
        use omnia_wasi_http::{HttpDefault, WasiHttp};
        use omnia_wasi_identity::{IdentityDefault, WasiIdentity};
        use omnia_wasi_keyvalue::{KeyValueDefault, WasiKeyValue};
        use omnia_wasi_messaging::{MessagingDefault, WasiMessaging};
        use omnia_wasi_otel::{OtelDefault, WasiOtel};
        use omnia_wasi_sql::{SqlDefault, WasiSql};
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
    }
    _ => {
        fn main() {}
    }
}
