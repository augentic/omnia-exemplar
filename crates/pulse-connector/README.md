# Pulse HTTP Connector

The Pulse HTTP connector receives Pulse data and posts it to the
`{env}-realtime-pulse.v1` topic.

Pulse data is received from track-side sensors that are triggered when a train
passes. This position data is used to help improve train location information
in underground stations (where GPS is not available).
