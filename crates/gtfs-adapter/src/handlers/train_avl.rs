//! Train AVL message filtering.

use common::fleet;
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Config, Error, HttpRequest, Identity, Publish, Result, StateStore};
use serde::Deserialize;

use crate::handlers::motion::{self, MotionMessage};

/// A Motion AVL message that should only be processed for Motion-tagged
/// train vehicles.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct TrainAvlMessage(pub MotionMessage);

impl<P> Operation<P> for TrainAvlMessage
where
    P: Provider + Config + HttpRequest + Identity + Publish + StateStore,
{
    type Error = Error;
    type Input = Self;
    type Output = ();

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<()> {
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

        motion::process(request, provider).await
    }
}
