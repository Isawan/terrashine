use sqlx::PgPool;
use terrashine::credhelper::database::DatabaseCredentials;
use terrashine::credhelper::CredentialHelper;

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
            .expect("Error occured")
            .expect("found empty value")
            == "password2",
        "Unexpected value"
    );
}

#[sqlx::test]
async fn test_insert_credential_helper_update_hostname(pool: Pool<Postgres>) {
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
            .expect("found empty value")
            == "password2",
        "Value not updated"
    );
    assert!(
        creds
            .get("terraform2.isawan.net")
            .await
            .expect("Error occured")
            .expect("found empty value")
            == "password1",
        "Unrelated row updated"
    );
}

#[sqlx::test]
async fn test_forget_credential(pool: Pool<Postgres>) {
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
            == None,
        "Found value when expected deletion"
    );
    assert!(
        creds
            .get("terraform2.isawan.net")
            .await
            .expect("Error occured")
            .expect("found empty value")
            == "password1",
        "Delete unrelated row detected"
    );
}
