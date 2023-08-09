# Terrashine

Terrashine is a terraform provider mirror[^1] implementation that works by automatically caching dependencies as providers are requested.

Use cases:

* Avoid rate-limits when actively developing (github has a 60 request per hour rate limit)
* Faster downloads of terraform providers, particularly in CI environments.
* Ensuring that terraform providers don't disappear if the source has been deleted.

## Installation

Terrashine is a rust binary. This is a project in early development, binaries are not currently published.

The project can be built with the command:

```
SQLX_OFFLINE=1 cargo build
```

## Dependencies

The following components are required to run terrashine

* PostgreSQL
* S3-compatible object storage
* TLS terminating reverse proxy (NGINX, HAProxy etc..)

## Notes

[^1]: Terrashine implements the [Provider Network Mirror Protocol](https://developer.hashicorp.com/terraform/internals/provider-network-mirror-protocol)
