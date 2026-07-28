# GTFS Adapter

The largest domain crate. Handles multiple message types for realtime train
position and passenger count data:

- **Motion messages** (`realtime-pulse-to-motion.v1`) — vehicle position and
  arrival/departure updates.
- **Train AVL** (`realtime-train-avl.v1`) — Motion AVL events filtered to
  Motion-tagged train vehicles before standard processing.
- **Passenger count** (`realtime-passenger-count.v1`) — onboard passenger
  count events, stored as occupancy status.

Also provides HTTP endpoints for vehicle info lookup and a "god mode" override
that allows manually forcing a vehicle onto a trip via the state store.
