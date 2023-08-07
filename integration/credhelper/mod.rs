use sqlx::PgPool;
use terrashine::credhelper::database::DatabaseCredentials;
use terrashine::credhelper::{Credential, CredentialHelper};

#[sqlx::test]
async fn test_insert_credential_helper(pool: PgPool) {
    let mut creds = DatabaseCredentials::new(pool);
    creds
        .store("terraform1.isawan.net".into(), "password1".into())
        .await
        .expect("Error occurred");
    creds
        .store("terraform2.isawan.net".into(), "password2".into())
        .await
        .expect("Error occurred");
    creds
        .store("terraform3.isawan.net".into(), "password3".into())
        .await
        .expect("Error occurred");

    assert!(
        creds
            .get("terraform2.isawan.net")
            .await
            .expect("Error occurred")
            == Credential::Entry(Some("password2".into())),
        "Unexpected value"
    );
}

#[sqlx::test]
async fn test_insert_credential_helper_update_hostname(pool: PgPool) {
    let mut creds = DatabaseCredentials::new(pool);
    creds
        .store("terraform1.isawan.net".into(), "password1".into())
        .await
        .expect("Error occurred");
    creds
        .store("terraform2.isawan.net".into(), "password1".into())
        .await
        .expect("Error occurred");
    creds
        .store("terraform1.isawan.net".into(), "password2".into())
        .await
        .expect("Error occurred");

    assert!(
        creds
            .get("terraform1.isawan.net")
            .await
            .expect("Error occured")
            == Credential::Entry(Some("password2".into())),
        "Value not updated"
    );
    assert!(
        creds
            .get("terraform2.isawan.net")
            .await
            .expect("Error occured")
            == Credential::Entry(Some("password1".into())),
        "Unrelated row updated"
    );
}

#[sqlx::test]
async fn test_forget_credential(pool: PgPool) {
    let mut creds = DatabaseCredentials::new(pool);
    creds
        .store("terraform1.isawan.net".into(), "password1".into())
        .await
        .expect("Error occurred");
    creds
        .store("terraform2.isawan.net".into(), "password1".into())
        .await
        .expect("Error occurred");
    creds
        .forget("terraform1.isawan.net")
        .await
        .expect("Error occurred deleting");

    assert!(
        creds
            .get("terraform1.isawan.net")
            .await
            .expect("Error occured")
            == Credential::NotFound,
        "Found value when expected deletion"
    );
    assert!(
        creds
            .get("terraform2.isawan.net")
            .await
            .expect("Error occured")
            != Credential::NotFound,
        "Delete unrelated row detected"
    );
}
