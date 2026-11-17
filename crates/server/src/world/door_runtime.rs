//! DoorManagingPlugin bound to the real native Scene replacement/collision and
//! pulse implementations. Player fields and effects are explicit host services.
use super::{
    door_option,
    doors::{self, Handle, Location, Result},
    scene, scenery, scenery_node,
};
pub trait Bindings<A: scene::Assets>: scene::Services<A> {
    type Properties: Clone;
    fn lowercase(&self, text: &str) -> String;
    fn player_location(&mut self, player: Option<usize>) -> Result<Option<Location>>;
    fn animate(&mut self, player: Option<usize>, id: i32, forced: bool) -> Result<()>;
    fn properties(&mut self, player: Option<usize>) -> Result<Option<Self::Properties>>;
    fn set_teleport(&mut self, properties: &Self::Properties, destination: Location) -> Result<()>;
}
pub struct Context<'a, A: scene::Assets, S: Bindings<A>> {
    pub scene: &'a mut scene::Scene<A>,
    pub services: &'a mut S,
}
fn error(e: impl std::fmt::Debug) -> doors::Error {
    doors::Error::Binding(format!("{e:?}"))
}
impl<A: scene::Assets, S: Bindings<A>> door_option::Context for Context<'_, A, S> {
    type Properties = S::Properties;
    fn register(&mut self, _: &str, _: usize) -> Result<()> {
        Err(doors::Error::Binding(
            "register through the owning scenery option registry".into(),
        ))
    }
    fn player_location(&mut self, p: Option<usize>) -> Result<Option<Location>> {
        self.services.player_location(p)
    }
    fn lowercase(&self, s: &str) -> String {
        self.services.lowercase(s)
    }
    fn audio(&mut self, p: Option<usize>, id: i32) -> Result<()> {
        self.services
            .audio(self.scene, p.ok_or(doors::Error::Null)?, id, None)
            .map_err(error)
    }
    fn animate(&mut self, p: Option<usize>, id: i32, forced: bool) -> Result<()> {
        self.services.animate(p, id, forced)
    }
    fn transform(&mut self, h: &Handle, id: i32) -> Result<Handle> {
        scenery_node::transform(&mut self.scene.definitions, h, id, None, None, None).map_err(error)
    }
    fn replace(&mut self, h: &Handle, new: Handle, ticks: Option<i32>) -> Result<()> {
        let mode = ticks.map_or(
            scenery::ReplaceMode::Swap {
                clip: true,
                permanent: false,
            },
            |ticks| scenery::ReplaceMode::Temporary { clip: true, ticks },
        );
        scenery::replace(
            &mut self.scene.context(self.services),
            Some(h),
            Some(&new),
            mode,
        )
        .map_err(error)?;
        Ok(())
    }
    fn teleport_permitted(&mut self, l: &Location) -> Result<bool> {
        self.scene
            .context(self.services)
            .teleport_permitted(Some(l))
            .map_err(error)
    }
    fn message(&mut self, p: Option<usize>, s: &str) -> Result<()> {
        self.services
            .message(self.scene, p.ok_or(doors::Error::Null)?, s)
            .map_err(error)
    }
    fn properties(&mut self, p: Option<usize>) -> Result<Option<Self::Properties>> {
        self.services.properties(p)
    }
    fn set_teleport(&mut self, p: &Self::Properties, l: Location) -> Result<()> {
        self.services.set_teleport(p, l)
    }
    fn door(&mut self, p: Option<usize>, h: &Handle) -> Result<()> {
        self.scene
            .context(self.services)
            .door(p.ok_or(doors::Error::Null)?, h)
            .map_err(error)
    }
    fn door_destination(&mut self, p: Option<usize>, h: &Handle) -> Result<Location> {
        doors::destination(
            &mut self.scene.context(self.services),
            p.ok_or(doors::Error::Null)?,
            h,
        )
    }
}
