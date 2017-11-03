use primitives::{Event, RawEvent};
use super::PlotID;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct PlotEvent {
    from: PlotID,
    to: PlotID,
    event: RawEvent
}
impl Event for PlotEvent {}