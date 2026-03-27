use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrowserEvent {
    Navigate { url: String },
    Click { index: usize },
    ClickCoordinate { x: f64, y: f64 },
    TypeText { index: usize, text: String },
    Scroll { direction: String, amount: u32 },
    SendKeys { keys: String },
    Screenshot,
    TabSwitch { index: usize },
    TabClose { index: usize },
    TabNew,
    PageLoaded { url: String },
    DomUpdated,
    Error { message: String },
    Log { message: String },
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<BrowserEvent>,
}

impl EventBus {
    pub fn new(buffer_size: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer_size);
        Self { sender }
    }

    pub fn publish(&self, event: BrowserEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BrowserEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}
