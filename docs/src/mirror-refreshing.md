# Mirror refreshing

To update the mirror dependencies, terrashine will refresh the provider metadata against the provider registry.
This is triggered by a request for the index listing after the refresh interval has been exceeded.
The refresh interval is configurable and defaults to an hour.

The refresh occurs is triggered by the request, however it is performed as a background job so the request that performs the request will return immediately with the stale data.
This design prevents outages of the upstream provider registry from affecting the performance of requests for existing mirrored providers.
The refresh job independent across multiple instances of terrashine is ran in a highly
available environment, so duplicate refresh jobs can occur as the number of nodes increase.

Only the version and provider metadata is updated, the actual provider artifacts are never modified after the initial download.
