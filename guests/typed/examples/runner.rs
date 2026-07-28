//! Host runner for the typed guest.
//!
//! Build the guest for `wasm32-wasip2` and run it with:
//!
//! ```shell
//! cargo build -p guest-typed --target wasm32-wasip2 --release
//! cargo run -p guest-typed --example runner -- run target/wasm32-wasip2/release/guest_typed.wasm
//! ```

cfg_if::cfg_if! {
    if #[cfg(not(target_arch = "wasm32"))] {
        use omnia_wasi_config::{WasiConfig, ConfigDefault};
        use omnia_wasi_http::{WasiHttp, HttpDefault};
        use omnia_wasi_identity::{WasiIdentity, IdentityDefault};
        use omnia_wasi_keyvalue::{WasiKeyValue, KeyValueDefault};
        use omnia_wasi_messaging::{WasiMessaging, MessagingDefault};
        use omnia_wasi_otel::{WasiOtel, OtelDefault};

        omnia::runtime!({
            hosts: {
                WasiConfig: ConfigDefault,
                WasiHttp: HttpDefault,
                WasiIdentity: IdentityDefault,
                WasiKeyValue: KeyValueDefault,
                WasiMessaging: MessagingDefault,
                WasiOtel: OtelDefault,
            }
        });
    } else {
        fn main() {}
    }
}
