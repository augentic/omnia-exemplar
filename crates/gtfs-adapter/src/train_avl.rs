//! Train AVL message filtering.

use common::{Capabilities, config, topic};
use omnia_sdk::api::Context;
use omnia_sdk::api::messaging::{self, consume};
use omnia_sdk::{Config, HttpRequest, Identity, Publish, Result, StateStore};
use serde::Deserialize;

use crate::fleet;
use crate::motion::{self, MotionMessage};

pub(crate) fn topics<P: Capabilities>(
    router: messaging::Router<P>, env: &str,
) -> messaging::Router<P> {
    router.route(config::topic_for(env, topic::TRAIN_AVL), consume(train_avl))
}

/// A Motion AVL message that should only be processed for Motion-tagged
/// train vehicles.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct TrainAvlMessage(pub MotionMessage);

#[tracing::instrument(skip_all)]
pub(crate) async fn train_avl<P>(input: TrainAvlMessage, context: Context<P>) -> Result<()>
where
    P: Config + HttpRequest + Identity + Publish + StateStore,
{
    let provider = context.provider();
    let request = input.0;

    // verify vehicle tag is 'motion'
    let Some(vehicle_id) = request.vehicle_id() else {
        tracing::debug!("no vehicle identifier found");
        return Ok(());
    };
    let Some(vehicle) = fleet::vehicle(vehicle_id, provider).await? else {
        tracing::debug!("vehicle info not found for {vehicle_id}");
        return Ok(());
    };
    if let Some(tag) = vehicle.tag.as_deref().map(str::to_lowercase)
        && tag != "motion"
    {
        tracing::debug!("vehicle tag {tag} did not match rules");
        return Ok(());
    }

    motion::motion(request, context).await
}
