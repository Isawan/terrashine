# Terrashine

![GitHub](https://img.shields.io/github/license/isawan/terrashine)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/isawan/terrashine/rust.yml)
[![codecov](https://codecov.io/gh/Isawan/terrashine/branch/main/graph/badge.svg?token=4LEQEEOMZT)](https://codecov.io/gh/Isawan/terrashine)

A terraform provider mirror implemented as a caching proxy.
Terrashine is a terraform provider mirror implementation that works by automatically caching dependencies as providers are requested.

Use cases:

* Avoid rate-limits when actively developing in ephemeral CI environments (github has a 60 request per hour rate limit)
* Faster downloads of terraform providers, particularly in CI environments.
* Ensuring that terraform providers don't disappear if the source has been deleted.

## Documentation

Terrashine is a compiled to a standalone binary making deployments easy.
Documentation for usage, deployment and administration can be found [here](https://isawan.github.io/terrashine/).

## Support

Raise any bugs or feature requests as tickets.

## Contributing

This is quite a new project so I'm very open to contributions!

## Authors and acknowledgment

* Isawan Millican
