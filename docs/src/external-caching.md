# External Caching

Terraform sets `Cache-Control` headers where possible to allow caching by external reverse proxies.
If caching is required, this should be achieved by configuring a reverse proxy to cache responses as appropriate.
Cache headers are sometimes not set in cases where caching may incorrect behavior by the terraform client.
For example: headers are not set in scenarios where caching could result in subsequent requests from the same client seeing inconsistent views of the available packages, resulting in an error when downloading packages.