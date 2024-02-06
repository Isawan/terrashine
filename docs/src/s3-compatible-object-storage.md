# Setting up S3-compatible object storage

Terrashine requires an S3 compatible storage to cache terraform providers.
It is currently tested against AWS S3 and Minio.
To set up AWS S3, please follow the AWS instruction to [create a bucket and obtain a set of credentials](https://docs.aws.amazon.com/AmazonS3/latest/userguide/GetStartedWithS3.html).

Terrashine requires a bucket to be created and a set of credentials to be available.
The AWS Rust SDK is used to authenticate to S3 so the credentials can be provided to the binary using any supported credential provider.
Most commonly, this can be provided using the `AWS_ACCESS_KEY_ID` and `AWS_SECRET_KEY_ID` environment variables.

Terrashine requires the following actions in the IAM policy:

* GetObject
* PutObject

For a non-AWS S3 compatible object storage, see the `docker-compose.yml` in the repository where an example minio integration is used.
In this case, a CLI flag `--s3-endpoint` can be used to point terrashine at an alternative URL.
