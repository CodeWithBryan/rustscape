//! Original MajorUpdateWorker.handleTickActions and Entity.tick phase ordering.
//! Repository/list iteration, concrete services and Player/NPC extensions remain
//! owning-world bindings; an error stops the body and the write flush still runs.
use super::doors::Location;
pub trait Context {
    type Error;
    fn now_millis(&mut self) -> Result<i64, Self::Error>;
    fn packets(&mut self) -> Result<(), Self::Error>;
    fn packet_time(&mut self, millis: i32) -> Result<(), Self::Error>;
    fn disconnects(&mut self) -> Result<(), Self::Error>;
    fn pulses(&mut self) -> Result<(), Self::Error>;
    fn listeners(&mut self) -> Result<(), Self::Error>;
    fn start(&mut self) -> Result<(), Self::Error>;
    fn run(&mut self) -> Result<(), Self::Error>;
    fn end(&mut self) -> Result<(), Self::Error>;
    fn pulse_world(&mut self) -> Result<(), Self::Error>;
    fn managers(&mut self) -> Result<(), Self::Error>;
    fn flush(&mut self) -> Result<(), Self::Error>;
    fn exception(&mut self, error: Self::Error) -> Result<(), Self::Error>;
}
pub fn handle<C: Context>(ctx: &mut C, skip_pulse_update: bool) -> Result<(), C::Error> {
    let body = (|| {
        let start = ctx.now_millis()?;
        ctx.packets()?;
        let elapsed = ctx.now_millis()?.wrapping_sub(start) as i32;
        ctx.packet_time(elapsed)?;
        ctx.disconnects()?;
        if !skip_pulse_update {
            ctx.pulses()?;
        }
        ctx.listeners()?;
        ctx.start()?;
        ctx.run()?;
        ctx.end()?;
        ctx.pulse_world()?;
        ctx.managers()
    })();
    let result = match body {
        Ok(()) => Ok(()),
        Err(e) => ctx.exception(e),
    };
    let finally = match ctx.flush() {
        Ok(()) => Ok(()),
        Err(e) => ctx.exception(e),
    };
    // A failure in finally overrides an already escaping catch failure in Java.
    finally.and(result)
}
pub trait EntityContext {
    type Error;
    fn pre_movement(&mut self) -> Result<(), Self::Error>;
    fn ticks(&mut self) -> Result<i32, Self::Error>;
    fn tick_event(&mut self, tick: i32) -> Result<(), Self::Error>;
    fn skills(&mut self) -> Result<(), Self::Error>;
    fn location(&self) -> Option<Location>;
    fn walking(&mut self) -> Result<(), Self::Error>;
    fn post_movement(&mut self, moved: bool) -> Result<(), Self::Error>;
    fn timers(&mut self) -> Result<(), Self::Error>;
    fn prepare_masks(&mut self) -> Result<(), Self::Error>;
}
pub fn entity<C: EntityContext>(ctx: &mut C) -> Result<(), C::Error> {
    entity_before_masks(ctx)?;
    ctx.prepare_masks()
}
pub(crate) fn entity_before_masks<C: EntityContext>(ctx: &mut C) -> Result<(), C::Error> {
    ctx.pre_movement()?;
    let ticks = ctx.ticks()?;
    ctx.tick_event(ticks)?;
    ctx.skills()?;
    let old = ctx.location().map_or([0, 0, 0], |l| l.coordinates());
    ctx.walking()?;
    let moved = ctx.location().is_none_or(|l| l.coordinates() != old);
    ctx.post_movement(moved)?;
    ctx.timers()
}
