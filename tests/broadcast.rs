use tokio::sync::broadcast;

#[tokio::test]
async fn message_reaches_multiple_subscribers() {
    let (sender, _) = broadcast::channel(10);
    let mut first_client = sender.subscribe();
    let mut second_client = sender.subscribe();

    sender.send("hello everyone").expect("receivers exist");

    assert_eq!(first_client.recv().await.unwrap(), "hello everyone");
    assert_eq!(second_client.recv().await.unwrap(), "hello everyone");
}
