use blockscape_core::primitives::Event;
use serde;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum PlotEvent {
    ExampleEvent(String)
}

impl Event for PlotEvent {}