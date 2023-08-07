# Database migrations

When upgrading or starting up terrashine for the first time, we need to run database migrations against the database.
For this, you'll need the [sqlx](https://github.com/launchbadge/sqlx) migration tool.
We can perform the migration with the following command.

``` bash
sqlx migrate run --database-url postgres://postgres:password@localhost/
```
