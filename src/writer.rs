use tokio::sync::mpsc::Receiver;

pub enum Message {
    FetchIndexUrl {},
    FetchVersionUrl {},
}

pub async fn writer(mut rx: Receiver<Message>) -> () {
    while let Some(cmd) = rx.recv().await {
        ()
    }
}
