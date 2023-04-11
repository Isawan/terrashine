# Mirror refreshing

To update the mirror dependencies, terrashine will refresh the provider metadata against the provider registry roughly once an hour for latest updates.
Only the version and provider metadata is updated, the actual provider artifacts are never modified after the inital download.
