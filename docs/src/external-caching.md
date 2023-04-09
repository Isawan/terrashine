# External Caching

Terraform sets `Control-Control` headers where possible to allow caching by external reverse proxies.
Currently the `/artifacts/+` endpoint is the only cachable entry without race conditions.
in a highly available setup, so it is the only endpoint to respond with `Content-Control` headers. The time on these headers is set short enough before the AWS presigned URL expires.
