# Starting terrashine

At this point, its expected that you now have a postgres database provisioned, an S3 endpoint for object storage and a reverse proxy for TLS termination.

Terrashine is configured via CLI flags and environment variables.
For a complete list of environment variables see:

```
terrashine --help
```

## Example

Here is an example of starting up terrashine using an S3 bucket named `terrashine-example-test-1111`, with credentials provided as environment variables `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`.
A TLS terminating reverse proxy hosted is on `example.com` in this setup.
Note that the `/mirror/v1/`  path is required in the URL to allow the backend server to serve up redirects correctly.

```
AWS_REGION=eu-west-1 AWS_ACCESS_KEY_ID=xxx AWS_SECRET_ACCESS_KEY=xxx RUST_LOG=info  ./terrashine  server --s3-bucket-name terrashine-example-test-1111  --http-redirect-url https://example.com/mirror/v1/
```
