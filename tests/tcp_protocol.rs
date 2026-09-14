use termlink::protocol::{self, ClientMessage, ServerMessage};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn exchanges_one_json_message_over_loopback_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let line = BufReader::new(reader)
            .lines()
            .next_line()
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            protocol::decode::<ClientMessage>(&line).unwrap(),
            ClientMessage::Chat {
                content: "hello over TCP".to_owned()
            }
        );

        let response = protocol::encode(&ServerMessage::Notice {
            message: "received".to_owned(),
        })
        .unwrap();
        writer.write_all(response.as_bytes()).await.unwrap();
        writer.write_all(b"\n").await.unwrap();
    });

    let stream = TcpStream::connect(address).await.unwrap();
    let (reader, mut writer) = stream.into_split();
    let request = protocol::encode(&ClientMessage::Chat {
        content: "hello over TCP".to_owned(),
    })
    .unwrap();
    writer.write_all(request.as_bytes()).await.unwrap();
    writer.write_all(b"\n").await.unwrap();

    let response = BufReader::new(reader)
        .lines()
        .next_line()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        protocol::decode::<ServerMessage>(&response).unwrap(),
        ServerMessage::Notice {
            message: "received".to_owned()
        }
    );

    server.await.unwrap();
}
