//! Explicit visual-input schedule, independent of any gameplay/combat handler.
use rs09_server::world::appearance::SpotAnimation;
use serde::Deserialize;
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Timeline {
    pub(crate) repeat_ticks: Option<u64>,
    pub(crate) events: Vec<Event>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct Event {
    pub(crate) tick: u64,
    #[serde(flatten)]
    pub(crate) graphic: SpotAnimation,
}
impl Timeline {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let timeline: Self = serde_json::from_slice(bytes)?;
        if timeline.repeat_ticks == Some(0) {
            return Err("spot probe repeatTicks must be positive".into());
        }
        let mut prior = 0;
        for event in &timeline.events {
            if event.tick <= prior {
                return Err("spot probe ticks must be positive and strictly increasing".into());
            }
            if timeline
                .repeat_ticks
                .is_some_and(|period| event.tick >= period)
            {
                return Err("spot probe event must precede repeatTicks".into());
            }
            prior = event.tick;
        }
        Ok(timeline)
    }
    pub(crate) fn at(&self, age: u64) -> Option<SpotAnimation> {
        let age = self.repeat_ticks.map_or(age, |period| age % period);
        self.events
            .iter()
            .find(|event| event.tick == age)
            .map(|event| event.graphic)
    }
}
