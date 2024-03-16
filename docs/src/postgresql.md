# PostgreSQL

Terrashine requires postgreSQL to store metadata associated with upstream registries and downloaded terraform providers.
Terrashine does not store any terraform providers inside of postgreSQL itself so the requirements are typically fairly light.

Please see postgreSQL's [excellent documentation](https://www.postgresql.org/docs/16/admin.html) to set up the database.

## Database migrations

When upgrading or starting up terrashine for the first time, we need to run database migrations against the database.
We can perform the migration with the following command.

``` bash
terrashine migrate --database-url postgresql://postgres:password@localhost:5432
```

This command should be executed from a checkout of the git repository associated with the version.

## Confirm migration succeeded

```bash
docker compose exec -it postgres psql postgresql://postgres:password@localhost:5432
```

```psql
\dt
\q
```