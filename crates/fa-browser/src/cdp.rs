use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;
type EventListeners = Arc<Mutex<HashMap<String, Vec<oneshot::Sender<Value>>>>>;

pub struct CdpClient {
    sender: Mutex<Box<dyn futures::Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Send + Unpin>>,
    pending: PendingMap,
    event_listeners: EventListeners,
}

impl CdpClient {
    pub async fn connect(ws_url: &str) -> Result<Self, anyhow::Error> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url).await?;
        let (write, read) = ws_stream.split();

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let event_listeners: EventListeners = Arc::new(Mutex::new(HashMap::new()));

        tokio::spawn(read_loop(read, pending.clone(), event_listeners.clone()));

        Ok(Self {
            sender: Mutex::new(Box::new(write)),
            pending,
            event_listeners,
        })
    }

    pub async fn execute(&self, method: &str, params: Value) -> Result<Value, anyhow::Error> {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        let message = json!({
            "id": id,
            "method": method,
            "params": params
        });

        let text = serde_json::to_string(&message)?;
        {
            let mut sender = self.sender.lock().await;
            sender.send(Message::Text(text.into())).await?;
        }

        tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| anyhow::anyhow!("CDP command timeout: {}", method))?
            .map_err(|_| anyhow::anyhow!("CDP response channel closed"))?
            .map_err(|e| anyhow::anyhow!("CDP error: {}", e))
    }

    pub async fn execute_unit(&self, method: &str, params: Value) -> Result<(), anyhow::Error> {
        self.execute(method, params).await.map(|_| ())
    }

    pub async fn wait_for_event(
        &self,
        event_name: &str,
        timeout: Duration,
    ) -> Result<Value, anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        {
            let mut listeners = self.event_listeners.lock().await;
            listeners
                .entry(event_name.to_string())
                .or_default()
                .push(tx);
        }

        tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| anyhow::anyhow!("Timeout waiting for event: {}", event_name))?
            .map_err(|_| anyhow::anyhow!("Event channel closed"))
    }
}

async fn read_loop<S>(
    mut read: futures::stream::SplitStream<S>,
    pending: PendingMap,
    event_listeners: EventListeners,
) where
    S: futures::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(data) = serde_json::from_str::<Value>(&text) {
                    if let Some(id) = data.get("id").and_then(|v| v.as_u64()) {
                        let mut map = pending.lock().await;
                        if let Some(tx) = map.remove(&id) {
                            let result = if data.get("error").is_some() {
                                Err(data["error"]["message"]
                                    .as_str()
                                    .unwrap_or("Unknown error")
                                    .to_string())
                            } else {
                                Ok(data["result"].clone())
                            };
                            let _ = tx.send(result);
                        }
                    } else if let Some(method) = data.get("method").and_then(|v| v.as_str()) {
                        let mut listeners = event_listeners.lock().await;
                        if let Some(senders) = listeners.remove(method) {
                            for tx in senders {
                                let _ = tx.send(data["params"].clone());
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                tracing::info!("CDP WebSocket connection closed");
                break;
            }
            Err(e) => {
                tracing::error!("CDP WebSocket read error: {}", e);
                break;
            }
            _ => {}
        }
    }
}
