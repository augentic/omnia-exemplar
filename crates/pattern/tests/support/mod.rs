//! The stem's component, booted over the backends a suite scripts.

use omnia::{Provides, Runtime};
use omnia_test::host::{Deployment, precompiled};
use omnia_wasi_config::WasiConfig;
use omnia_wasi_http::WasiHttp;
use omnia_wasi_identity::WasiIdentity;
use omnia_wasi_keyvalue::WasiKeyValue;
use omnia_wasi_messaging::WasiMessaging;
use omnia_wasi_otel::WasiOtel;
use omnia_wasi_sql::WasiSql;

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

/// Boots the stem's component over `backends`, with every host it imports.
pub async fn boot<B>(backends: B) -> Runtime<B>
where
    B: Provides<WasiConfig>
        + Provides<WasiHttp>
        + Provides<WasiIdentity>
        + Provides<WasiKeyValue>
        + Provides<WasiMessaging>
        + Provides<WasiOtel>
        + Provides<WasiSql>,
{
    let guest = precompiled(PATTERN_GUEST).expect("guest precompiles");
    Deployment::new()
        .guest("pattern", guest)
        .boot(backends, |deployment| {
            deployment.host::<WasiConfig>()?;
            deployment.host::<WasiHttp>()?;
            deployment.host::<WasiIdentity>()?;
            deployment.host::<WasiKeyValue>()?;
            deployment.host::<WasiMessaging>()?;
            deployment.host::<WasiOtel>()?;
            deployment.host::<WasiSql>()?;
            Ok(())
        })
        .await
        .expect("runtime boots")
}
