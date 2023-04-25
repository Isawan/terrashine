use rand::{
    distributions::{self, Alphanumeric},
    thread_rng, Rng,
};
use sqlx::{postgres::PgConnectOptions, query, PgPool, Postgres, QueryBuilder};

struct TempDatabase {
    connection_string: String,
}

const VALID_DB_NAME_CHARACTERS: &'static str = "abcdefghijklmnopqrstuvwxyz0123456789";
const VALID_DB_NAME_FIRST_CHARACTER: &'static str = "abcdefghijklmnopqrstuvwxyz";

struct ValidatedDBName<'a>(&'a str);

impl ValidatedDBName<'_> {
    fn parse(s: &str) -> Option<Self> {
        let valid_first_character = s
            .chars()
            .nth(0)
            .filter(|c| VALID_DB_NAME_FIRST_CHARACTER.contains(*c));
        let valid_chars = s.chars().all(|c| VALID_DB_NAME_CHARACTERS.contains(c));
        if valid_first_character.is_some() && valid_chars {
            Some(Self(s))
        } else {
            None
        }
    }
}

impl AsRef<str> for ValidatedDBName<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl TempDatabase {
    fn new(connection_string: &str, template: ValidatedDBName) -> Self {
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

        // Effectively doing "create database $1 with template $2"
        // but we can't use parameterized query on database names in postgresql
        let query: query::Query<Postgres, _> = QueryBuilder::new("create database")
            .push(&db_name.as_ref())
            .push("with template")
            .push(template.as_ref())
            .build();
        query.execute(db);
        let connect_options = format!("postgresql://postgres:password@localhost:{}/{}", option.),
        TempDatabase { connect_options }
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {}
}
