# Terrashine

Terrashine is a terraform provider mirror[^1] implementation that works by automatically caching dependencies as providers are requested.

Use cases:

* Avoid rate-limits when actively developing (github has a 60 request per hour rate limit)
* Faster downloads of terraform providers, particularly in CI environments.
* Ensuring that terraform providers don't disappear if the source has been deleted.

## Installation

Terrashine is a deployed as a standalone binary.
Binary releases for x86-64 are published can be found on the [releases](https://github.com/Isawan/terrashine/releases) page.

Alternatively, the project can be built from source with the following command:

On Debian-based Linux:
```
sudo apt install musl-tools
rustup target add x86_64-unknown-linux-musl
```

## Building
```sh
SQLX_OFFLINE=1 cargo build --release
```

Once built, the binary can be found at `./target/x86_64-unknown-linux-musl/release/terrashine`

## Install
```sh
cargo install --path .
which terrashine
```

See the `--help` for more information:

```sh
terrashine --help
```

# Client configuration

Once terrashine is all setup, the terraform client needs to be configured to use the mirror.
This can be done a terraform configuration file entry.

* On linux and MacOS, a `.terraformrc` file should created in the home directory.
* On Windows `terraform.rc` file should be created in the `%APPDATA%` directory.

This file should contain configuration to point terraform at the installed provider mirror.

```
provider_installation {
  network_mirror {
    url = "https://example.com/mirror/v1/"
  }
}
```

For more information on the terraform configuration file, see the [CLI Configuration File](https://developer.hashicorp.com/terraform/cli/config/config-file#provider-installation) docs from hashicorp.

## High availability

Multiple instances of terrashine can be deployed to support high availability.
Simply point the instances at the same storage layer.

## Metrics

Terrashine supports the /metrics endpoint to export metrics in the prometheus format.
This can be ingested via prometheus or any other monitoring tool that understands
the prometheus exposition format.

## Dependencies

The following components are required to run terrashine

* PostgreSQL
* S3-compatible object storage
* TLS terminating reverse proxy (NGINX, HAProxy etc..)

## Notes

[^1]: Terrashine implements the [Provider Network Mirror Protocol](https://developer.hashicorp.com/terraform/internals/provider-network-mirror-protocol)
