# Private Registry Authentication

To insert credentials for private registries, the auth token can be updated with an API call.

``` bash
curl  -X POST \
    -d '{ "data": { "token": "xxxx"} }' \
    -H 'Content-Type: application/json' \
    https://localhost:9443/api/v1/credentials/example.com
```

Likewise, to delete a credential, the auth token can be deleted via a `DELETE` request.

```
curl  -X DELETE https://localhost:9443/api/v1/credentials/example.com
```
