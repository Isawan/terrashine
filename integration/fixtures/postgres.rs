use rand::{
    distributions::{self, Alphanumeric},
    thread_rng, Rng,
};
use sqlx::{postgres::PgConnectOptions, query, PgPool, Postgres, QueryBuilder};
use tokio::task::yield_now;

pub(crate) struct TempDatabase {
    connection_string: String,
    db_name: ValidatedDBName,
}

const VALID_DB_NAME_CHARACTERS: &'static str = "abcdefghijklmnopqrstuvwxyz0123456789";
const VALID_DB_NAME_FIRST_CHARACTER: &'static str = "abcdefghijklmnopqrstuvwxyz";

#[derive(Clone)]
struct ValidatedDBName(String);

impl ValidatedDBName {
    fn parse(s: &str) -> Option<Self> {
        let valid_first_character = s
            .chars()
            .nth(0)
            .filter(|c| VALID_DB_NAME_FIRST_CHARACTER.contains(*c));
        let valid_chars = s.chars().all(|c| VALID_DB_NAME_CHARACTERS.contains(c));
        if valid_first_character.is_some() && valid_chars {
            Some(Self(s.to_string()))
        } else {
            None
        }
    }
}

impl AsRef<str> for ValidatedDBName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl TempDatabase {
    pub(crate) fn new() -> Self {
        tokio_test::block_on(async {
            let connection_string = "postgresql://postgres:password@localhost:5432/";
            let db = PgPool::connect(connection_string)
                .await
                .expect("Could not create client");

            let mut db_name_bytes = vec![b'd']; // required because of first character rule
            db_name_bytes.extend(
                thread_rng()
                    .sample_iter(
                        distributions::Slice::new(&VALID_DB_NAME_CHARACTERS.as_bytes()).unwrap(),
                    )
                    .take(16)
                    .map(|x| *x),
            );

            let db_name = ValidatedDBName::parse(
                String::from_utf8(db_name_bytes)
                    .expect("Not UTF-8 string")
                    .as_str(),
            )
            .unwrap();

            // Effectively doing "create database $1"
            // but we can't use parameterized query on database names in postgresql
            let mut query = QueryBuilder::new("create database ");
            query.push(&db_name.as_ref());
            let query: query::Query<Postgres, _> = query.build();
            query.execute(&db).await.expect("Could not create database");

            sqlx::migrate!()
                .run(&db)
                .await
                .expect("Could not run migration");

            let connection_string = format!(
                "postgresql://postgres:password@localhost:5432/{}",
                db_name.as_ref()
            );
            TempDatabase {
                connection_string,
                db_name,
            }
        })
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        tokio_test::block_on(async {
            let connection_string = self.connection_string.clone();
            let db_name = self.db_name.clone();
            let handle = tokio::runtime::Handle::current();
            let db = PgPool::connect(&connection_string)
                .await
                .expect("Could not create client");

            // Effectively doing "drop database $1"
            // but we can't use parameterized query on database names in postgresql
            let mut query = QueryBuilder::new("drop database");
            query.push(db_name.as_ref());
            let query: query::Query<Postgres, _> = query.build();
            query.execute(&db).await.expect("Could not create database");
        });
    }
}
