//! Train AVL message filtering.

use acme_common::fleet;
use omnia_guest::api::Context;
use omnia_guest::{Config, HttpRequest, Identity, Publish, Result, StateStore};
use serde::Deserialize;

use crate::handlers::motion::{self, MotionMessage};

/// A Motion AVL message that should only be processed for Motion-tagged
/// train vehicles.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct TrainAvlMessage(pub MotionMessage);

#[omnia_guest::handler]
#[tracing::instrument(skip_all)]
async fn train_avl_message<P>(input: TrainAvlMessage, context: Context<'_, P>) -> Result<()>
where
    P: Config + HttpRequest + Identity + Publish + StateStore,
{
    let provider = context.provider;
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

    motion::motion_message(request, context).await
}
