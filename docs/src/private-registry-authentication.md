# Private Registry Authentication

To authenticate against private registries, the auth token can be inserted into the `terraform_registry_host` postgres table.

``` sql
insert into "terraform_registry_host" ("hostname", "auth_token")
    values ("example-private-registry.com", "xxxxxx")
```

An API call to do this is planned.
