//! Shared code for the TermLink server and client.

pub mod auth;
pub mod commands;
pub mod database;
pub mod protocol;

/// The human-readable application name.
pub const APP_NAME: &str = "TermLink";

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn broadcast_reaches_multiple_subscribers() {
        let (sender, _) = broadcast::channel(10);
        let mut first_client = sender.subscribe();
        let mut second_client = sender.subscribe();

        sender.send("hello everyone").expect("receivers exist");

        assert_eq!(first_client.recv().await.unwrap(), "hello everyone");
        assert_eq!(second_client.recv().await.unwrap(), "hello everyone");
    }
}
