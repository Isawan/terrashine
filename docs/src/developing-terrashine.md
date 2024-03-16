# Developing terrashine

Here is an end-to-end example of running terrashine for local development

1. Ensure dependencies are installed on host machine:
- terraform
- aws-cli
- mkcert
- docker
- docker-compose plugin
- rust

2. Build and install terrashine
```bash
SQLX_OFFLINE=1 cargo build --release
cargo install --path .
which terrashine
```

3. Generate localhost cert and install cert into host machine trust stores
```bash
pushd resources/test/certs
mkcert localhost
mkcert -install
popd
```

4. Copy generated cert to nginx config dir
```bash
pushd resources/test/nginx
cat $(pwd)/../certs/localhost.pem $(pwd)/../certs/localhost-key.pem > ./localhost.pem
popd
```

5. Create terraform configuration
```bash
cat << EOF > ~/.terraformrc
provider_installation {
  network_mirror {
    url = "https://localhost:9443/mirror/v1/"
  }
}
EOF
```

6. Configure aws cli with minio profile with creds set in docker-compose.yml file
```bash
cat << EOF >> ~/.aws/config
[profile minio]
region=eu-west-2
output=json
EOF

cat << EOF >> ~/.aws/credentials
[minio]
aws_access_key_id=minioadmin
aws_secret_access_key=minioadmin
EOF
```


7. Start docker compose services
```bash
sed -i '/generate-certificates:/,/^$/ s/^/# /' docker-compose.yml 

sed -i '/nginx:/,/}/s/    depends_on:/#    depends_on:/; /- generate-certificates/s/^/#/' docker-compose.yml

docker compose up
```

8. Run database migrations
```bash
terrashine migrate --database-url postgresql://postgres:password@localhost:5432
```

9. Confirm database migrations ran successfully
```bash
docker compose exec -it postgres psql postgresql://postgres:password@localhost:5432
```

```psql
\dt
\q
```


10. Confirm terrashine bucket was successfully created by the aws-cli startup command (executed by docker compose)
```bash
AWS_PROFILE=minio aws s3 ls --endpoint-url http://localhost:9000
```


11. Start terrashine server
```bash
AWS_PROFILE=minio RUST_LOG=info  terrashine server --s3-bucket-name terrashine --s3-endpoint http://localhost:9000 --http-redirect-url https://localhost:9443/mirror/v1/ 
```


12. Test the network mirror

```bash
AWS_PROFILE=minio aws s3api list-objects --bucket terrashine --endpoint-url http://localhost:9000
```

```bash
pushd resources/test/terraform/random-import-stack
terraform init -backend=false
popd
```
Output:

```json
{
    "RequestCharged": null
}
```

```bash
AWS_PROFILE=minio aws s3api list-objects --bucket terrashine --endpoint-url http://localhost:9000
```

Output:

```json
{
    "Contents": [
        {
            "Key": "artifacts/3",
            "LastModified": "2024-03-16T01:09:08.152000+00:00",
            "ETag": "\"5167941e0cb7772b814c078ef1eae5dd-1\"",
            "Size": 4741003,
            "StorageClass": "STANDARD",
            "Owner": {
                "DisplayName": "minio",
                "ID": "02d6176db174dc93cb1b899f7c6078f08654445fe8cf1b6ce98d8855f66bdbf4"
            }
        },
        {
            "Key": "artifacts/4",
            "LastModified": "2024-03-16T01:09:12.575000+00:00",
            "ETag": "\"37463ad75656f76c963d2bd310d449f2-8\"",
            "Size": 77926785,
            "StorageClass": "STANDARD",
            "Owner": {
                "DisplayName": "minio",
                "ID": "02d6176db174dc93cb1b899f7c6078f08654445fe8cf1b6ce98d8855f66bdbf4"
            }
        }
    ],
    "RequestCharged": null
}

```

## Cleanup

```bash
docker compose down

pushd resources/test/certs
mkcert -uninstall
popd

rm -f ~/.terraformrc
```