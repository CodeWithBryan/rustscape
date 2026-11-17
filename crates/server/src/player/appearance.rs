//! Original Appearance, BodyPart, Gender and mutable per-world static defaults.
//! Canonical player/render/content operations are explicit receiver services.
use crate::{
    definitions::{
        item::{Registry, Shared, WeakShared},
        npc,
        property::Property,
    },
    inventory::{
        self as inv,
        container::{Container, Context as ContainerContext},
        item::{Handle as ItemHandle, Item},
    },
    world::{
        animation::{Animation, AnimationFields, Priority},
        player_walk::NodeHandle,
    },
};
use rs09_game_core::{java_json::Json, java_string::parse_i32};
pub type Part = Shared<BodyPart>;
pub type Parts = Shared<[Option<Part>; 7]>;
pub type Animations = Shared<Vec<Option<Shared<AnimationFields>>>>;
pub const DEFAULT_ANIMATIONS: [i32; 7] = [808, 823, 819, 820, 821, 822, 824];
#[derive(Debug)]
pub enum Error {
    Null,
    Index,
    Type,
    Number,
    Inventory(inv::Error),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<inv::Error> for Error {
    fn from(e: inv::Error) -> Self {
        Self::Inventory(e)
    }
}
fn object(v: &Json) -> Result<()> {
    match v {
        Json::Object(_) => Ok(()),
        Json::Null => Err(Error::Null),
        _ => Err(Error::Type),
    }
}
fn required<'a>(v: &'a Json, key: &str) -> Result<&'a Json> {
    v.get(key)
        .filter(|v| !matches!(v, Json::Null))
        .ok_or(Error::Null)
}
fn integer(v: &Json) -> Result<i32> {
    parse_i32(&v.text()).ok_or(Error::Number)
}
#[derive(Debug, Default)]
pub struct BodyPart {
    pub look: i32,
    pub color: i32,
}
impl BodyPart {
    pub fn new(look: i32, color: i32) -> Self {
        Self { look, color }
    }
    pub fn parse(&mut self, v: &Json) -> Result<()> {
        object(v)?;
        self.look = integer(required(v, "look")?)?;
        self.color = integer(required(v, "color")?)?;
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gender {
    Male,
    Female,
}
impl Gender {
    pub fn byte(self) -> i8 {
        i8::from(self == Self::Female)
    }
    pub fn as_byte(value: i8) -> Self {
        if value == 0 { Self::Male } else { Self::Female }
    }
}
#[derive(Clone)]
pub struct Factory {
    identity: Shared<()>,
    defaults: [Parts; 2],
    animations: Shared<[i32; 7]>,
    attacks: Animations,
}
impl Default for Factory {
    fn default() -> Self {
        let defaults = [[0, 10, 18, 26, 33, 36, 42], [45, 1000, 56, 61, 68, 70, 79]]
            .map(|looks| Shared::new(looks.map(|look| Some(Shared::new(BodyPart::new(look, 0))))));
        Self {
            identity: Shared::new(()),
            defaults,
            animations: Shared::new(DEFAULT_ANIMATIONS),
            attacks: Shared::new(
                [422, 423, 422, 422]
                    .into_iter()
                    .map(|id| Some(Animation::with(id, 0, Some(Priority::High)).shared()))
                    .collect(),
            ),
        }
    }
}
impl Factory {
    pub fn same(&self, other: &Self) -> bool {
        self.identity.same(&other.identity)
    }
    pub fn gender_cache(&self, gender: Gender) -> Parts {
        self.defaults[gender.byte() as usize].clone()
    }
    pub fn animation_defaults(&self) -> Shared<[i32; 7]> {
        self.animations.clone()
    }
    pub fn default_attacks(&self) -> Animations {
        self.attacks.clone()
    }
    pub fn generate(&self, gender: Gender) -> Result<Parts> {
        let defaults = self.gender_cache(gender);
        let mut out = std::array::from_fn(|_| None);
        for (i, p) in out.iter_mut().enumerate() {
            let source = defaults.read()[i].clone().ok_or(Error::Null)?;
            let look = source.read().look;
            *p = Some(Shared::new(BodyPart::new(look, 0)));
        }
        Ok(Shared::new(out))
    }
}
struct Fields {
    player: Option<NodeHandle>,
    factory: Factory,
    animations: Shared<[i32; 7]>,
    icons: Shared<[i32; 2]>,
    body: Shared<[i32; 14]>,
    parts: Parts,
    gender: Option<Gender>,
    npc_id: i32,
    render: i32,
    riding: bool,
}
#[derive(Clone)]
pub struct Appearance(Shared<Fields>);
pub struct WeakAppearance(WeakShared<Fields>);
impl WeakAppearance {
    pub fn upgrade(&self) -> Option<Appearance> {
        self.0.upgrade().map(Appearance)
    }
}
#[derive(Clone, Copy, Debug)]
pub enum CombatAnimation {
    Attack,
    Defence,
    Death,
}
pub trait BodyContext {
    type Render;
    fn equipment(&mut self, player: &NodeHandle) -> Result<Container>;
    fn render(&mut self, player: &NodeHandle) -> Result<Self::Render>;
    fn prepared(&mut self, render: Self::Render) -> Result<bool>;
    fn set_prepared(&mut self, render: Self::Render, value: bool) -> Result<()>;
}
pub trait Context: BodyContext + ContainerContext {
    type Settings;
    type Properties;
    fn npc(&mut self, id: i32) -> Result<npc::Handle>;
    fn size(&mut self, player: &NodeHandle, value: i32) -> Result<()>;
    fn settings(&mut self, player: &NodeHandle) -> Result<Self::Settings>;
    fn attack_style(&mut self, settings: Self::Settings) -> Result<i32>;
    fn properties(&mut self, player: &NodeHandle) -> Result<Self::Properties>;
    fn combat_animation(
        &mut self,
        properties: Self::Properties,
        slot: CombatAnimation,
        animation: Option<Animation>,
    ) -> Result<()>;
    fn update_appearance(&mut self, player: &NodeHandle) -> Result<()>;
    fn appearance(&mut self, player: &NodeHandle) -> Result<Appearance>;
    fn animate(&mut self, player: &NodeHandle, animation: Animation) -> Result<bool>;
    fn attribute(
        &mut self,
        player: &NodeHandle,
        key: &str,
        default: Option<i32>,
    ) -> Result<Property>;
}
fn property(item: &ItemHandle, key: &str) -> Result<Option<Property>> {
    let d = item.read().definition.clone().ok_or(Error::Null)?;
    let p = d.read().handlers.get(key).filter(|p| !p.is_null()).cloned();
    Ok(p)
}
fn number(item: &ItemHandle, key: &str, default: i32) -> Result<i32> {
    match property(item, key)? {
        None => Ok(default),
        Some(Property::Int(v)) => Ok(v),
        _ => Err(Error::Type),
    }
}
fn flag(item: &ItemHandle, key: &str) -> Result<bool> {
    match property(item, key)? {
        None => Ok(false),
        Some(Property::Bool(v)) => Ok(v),
        _ => Err(Error::Type),
    }
}
impl Appearance {
    pub(crate) fn ownership(&self) -> (usize, usize, Option<NodeHandle>) {
        (self.0.identity(), self.0.strong_count(), self.player())
    }
    pub fn new(player: Option<NodeHandle>, factory: Factory) -> Result<Self> {
        let parts = factory.generate(Gender::Male)?;
        Ok(Self(Shared::new(Fields {
            player,
            factory,
            parts,
            animations: Shared::new(DEFAULT_ANIMATIONS),
            icons: Shared::new([-1, -1]),
            body: Shared::new([0; 14]),
            gender: Some(Gender::Male),
            npc_id: -1,
            render: 1426,
            riding: false,
        })))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn downgrade(&self) -> WeakAppearance {
        WeakAppearance(self.0.downgrade())
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.0.read().player.clone()
    }
    pub fn factory(&self) -> Factory {
        self.0.read().factory.clone()
    }
    pub fn parts(&self) -> Parts {
        self.0.read().parts.clone()
    }
    pub fn animations(&self) -> Shared<[i32; 7]> {
        self.0.read().animations.clone()
    }
    pub fn body(&self) -> Shared<[i32; 14]> {
        self.0.read().body.clone()
    }
    pub fn gender(&self) -> Option<Gender> {
        self.0.read().gender
    }
    pub fn is_male(&self) -> bool {
        self.gender() == Some(Gender::Male)
    }
    pub fn npc_id(&self) -> i32 {
        self.0.read().npc_id
    }
    pub fn is_npc(&self) -> bool {
        self.npc_id() != -1
    }
    pub fn riding(&self) -> bool {
        self.0.read().riding
    }
    pub fn set_riding(&self, v: bool) {
        self.0.write().riding = v;
    }
    pub fn skull_icon(&self) -> i32 {
        self.0.read().icons.read()[0]
    }
    pub fn head_icon(&self) -> i32 {
        self.0.read().icons.read()[1]
    }
    pub fn set_skull_icon(&self, v: i32) {
        self.0.read().icons.write()[0] = v;
    }
    pub fn set_head_icon(&self, v: i32) {
        self.0.read().icons.write()[1] = v;
    }
    pub fn cached_render(&self) -> i32 {
        self.0.read().render
    }
    pub fn part(&self, index: usize) -> Result<Option<Part>> {
        self.parts().read().get(index).cloned().ok_or(Error::Index)
    }
    pub fn hair(&self) -> Option<Part> {
        self.parts().read()[0].clone()
    }
    pub fn beard(&self) -> Option<Part> {
        self.parts().read()[1].clone()
    }
    pub fn torso(&self) -> Option<Part> {
        self.parts().read()[2].clone()
    }
    pub fn arms(&self) -> Option<Part> {
        self.parts().read()[3].clone()
    }
    pub fn wrists(&self) -> Option<Part> {
        self.parts().read()[4].clone()
    }
    pub fn skin(&self) -> Option<Part> {
        self.wrists()
    }
    pub fn legs(&self) -> Option<Part> {
        self.parts().read()[5].clone()
    }
    pub fn feet(&self) -> Option<Part> {
        self.parts().read()[6].clone()
    }
    fn look(&self, index: usize) -> Result<i32> {
        let p = self.part(index)?.ok_or(Error::Null)?;
        let value = p.read().look;
        Ok(value)
    }
    pub fn animation(&self, index: usize) -> Result<i32> {
        self.animations()
            .read()
            .get(index)
            .copied()
            .ok_or(Error::Index)
    }
    pub fn set_animation(&self, index: usize, id: i32) -> Result<()> {
        *self
            .animations()
            .write()
            .get_mut(index)
            .ok_or(Error::Index)? = id;
        Ok(())
    }
    pub fn set_gender(&self, gender: Option<Gender>) -> Result<()> {
        self.0.write().gender = gender;
        let parts = self.factory().generate(gender.ok_or(Error::Null)?)?;
        self.0.write().parts = parts;
        Ok(())
    }
    pub fn change_gender(&self, ctx: &mut impl Context, gender: Option<Gender>) -> Result<()> {
        self.set_gender(gender)?;
        self.sync(ctx)
    }
    pub fn parse(&self, v: &Json) -> Result<()> {
        object(v)?;
        let receiver = self.gender();
        let byte = i8::try_from(integer(required(v, "gender")?)?).map_err(|_| Error::Number)?;
        receiver.ok_or(Error::Null)?;
        self.0.write().gender = Some(Gender::as_byte(byte));
        let value = v.get("appearance_cache").unwrap_or(&Json::Null);
        let values = match value {
            Json::Array(v) => v,
            Json::Null => return Err(Error::Null),
            _ => return Err(Error::Type),
        };
        let parts = self.parts();
        for i in 0..7 {
            let receiver = parts.read()[i].clone();
            let value = values.get(i).ok_or(Error::Index)?;
            object(value)?;
            receiver.ok_or(Error::Null)?.write().parse(value)?;
        }
        Ok(())
    }
    pub fn copy(&self, other: Option<&Self>) -> Result<()> {
        let other = other.ok_or(Error::Null)?;
        let gender = other.gender();
        self.0.write().gender = gender;
        let parts = self.parts();
        let source = other.parts();
        for i in 0..7 {
            let p = source.read()[i].clone();
            parts.write()[i] = p;
        }
        let animations = self.animations();
        let source = other.animations();
        for i in 0..7 {
            let a = source.read()[i];
            animations.write()[i] = a;
        }
        let render = other.cached_render();
        self.0.write().render = render;
        Ok(())
    }
    pub fn default_animations(&self) {
        let defaults = self.factory().animation_defaults();
        let animations = self.animations();
        for i in 0..7 {
            let v = defaults.read()[i];
            animations.write()[i] = v;
        }
        self.0.write().render = 1426;
    }
    pub fn draw_item(&self, part: i32, item: Option<&ItemHandle>) -> Result<()> {
        let item = item.ok_or(Error::Null)?;
        let d = item.read().definition.clone().ok_or(Error::Null)?;
        let id = d.read().equip_id;
        *self
            .body()
            .write()
            .get_mut(part as usize)
            .ok_or(Error::Index)? = id.wrapping_add(0x8000);
        Ok(())
    }
    pub fn draw_clothes(&self, part: i32, id: i32) -> Result<()> {
        *self
            .body()
            .write()
            .get_mut(part as usize)
            .ok_or(Error::Index)? = id.wrapping_add(0x100);
        Ok(())
    }
    pub fn clear_body_part(&self, part: i32) -> Result<()> {
        *self
            .body()
            .write()
            .get_mut(part as usize)
            .ok_or(Error::Index)? = 0;
        Ok(())
    }
    pub fn hat_clipping(&self) -> Result<()> {
        let look = self.look(0)?;
        let bald = look == if self.is_male() { 0 } else { 45 };
        let look = if bald {
            self.look(0)?
        } else if self.is_male() {
            5
        } else {
            51
        };
        self.body().write()[8] = look.wrapping_add(0x100);
        Ok(())
    }
    pub fn sync(&self, ctx: &mut impl Context) -> Result<()> {
        ctx.update_appearance(self.player().as_ref().ok_or(Error::Null)?)
    }
    pub fn set_render_animation(
        &self,
        ctx: &mut impl Context,
        animation: Option<&Animation>,
    ) -> Result<()> {
        self.0.write().render = animation.ok_or(Error::Null)?.id();
        self.sync(ctx)
    }
    pub fn render_animation(&self, ctx: &mut impl Context) -> Result<i32> {
        let player = self.player().ok_or(Error::Null)?;
        if !ctx
            .attribute(&player, "render-anim-override", None)?
            .is_null()
        {
            return match ctx.attribute(
                &player,
                "render-anim-override",
                Some(self.cached_render()),
            )? {
                Property::Int(v) => Ok(v),
                Property::Null => Err(Error::Null),
                _ => Err(Error::Type),
            };
        }
        Ok(self.cached_render())
    }
    pub fn set_animations(&self, ctx: &mut impl Context) -> Result<()> {
        if self.npc_id() == -1 {
            let player = self.player().ok_or(Error::Null)?;
            let c = ctx.equipment(&player)?;
            let weapon = c.get(3);
            if self.riding() {
                self.set_riding(false);
            }
            if weapon
                .as_ref()
                .map(|w| property(w, "walk_anim"))
                .transpose()?
                .flatten()
                .is_none()
            {
                self.default_animations();
            } else {
                let weapon = weapon.as_ref().unwrap();
                for (slot, key, default) in [
                    (0, "stand_anim", 808),
                    (1, "stand_turn_anim", 823),
                    (2, "walk_anim", 819),
                    (6, "run_anim", 824),
                    (3, "turn180_anim", 820),
                    (4, "turn90cw_anim", 821),
                    (5, "turn90ccw_anim", 822),
                ] {
                    self.set_animation(slot, number(weapon, key, default)?)?;
                }
                self.0.write().render = number(weapon, "render_anim", 1426)?;
            }
            if weapon.is_some_and(|w| w.read().id() == 12842) {
                for (slot, id) in [
                    (0, 8964),
                    (2, 8961),
                    (6, 8963),
                    (3, 8963),
                    (5, 8963),
                    (4, 8963),
                    (1, 8963),
                ] {
                    self.set_animation(slot, id)?;
                }
            }
        } else {
            let d = ctx.npc(self.npc_id())?;
            self.0.write().render = d.read().render_animation_id;
            self.set_animation(0, d.read().stand_animation)?;
            let mut turn = d.read().turn_animation;
            if turn < 1 {
                turn = d.read().walk_animation;
            }
            self.set_animation(1, turn)?;
            self.set_animation(2, d.read().walk_animation)?;
            self.set_animation(6, d.read().walk_animation)?;
            for (slot, value) in [
                (3, d.read().turn180_animation),
                (4, d.read().turn_cw_animation),
                (5, d.read().turn_ccw_animation),
            ] {
                self.set_animation(slot, if value > 0 { value } else { turn })?;
            }
        }
        Ok(())
    }
    pub fn transform_npc(&self, ctx: &mut impl Context, id: i32) -> Result<()> {
        self.0.write().npc_id = id;
        self.set_animations(ctx)?;
        let owner = self.player();
        if id == -1 {
            let player = owner.as_ref().ok_or(Error::Null)?;
            ctx.size(player, 1)?;
            let mut animations = self.factory().default_attacks();
            let equipment = ctx.equipment(player)?;
            if equipment.get(3).is_some() {
                let equipment = ctx.equipment(player)?;
                let weapon = equipment.get(3).ok_or(Error::Null)?;
                if let Some(p) = property(&weapon, "attack_anims")? {
                    animations = match p {
                        Property::Animations(a) => a,
                        _ => return Err(Error::Type),
                    };
                }
            }
            let settings = ctx.settings(player)?;
            let mut index = ctx.attack_style(settings)?;
            if index < 0 || index as usize >= animations.read().len() {
                index = 0;
            }
            let properties = ctx.properties(player)?;
            let animation = animations
                .read()
                .get(index as usize)
                .cloned()
                .ok_or(Error::Index)?
                .map(Animation::from_shared);
            ctx.combat_animation(properties, CombatAnimation::Attack, animation)?;
            let properties = ctx.properties(player)?;
            ctx.combat_animation(
                properties,
                CombatAnimation::Defence,
                Some(Animation::new(404)),
            )?;
            let properties = ctx.properties(player)?;
            ctx.combat_animation(
                properties,
                CombatAnimation::Death,
                Some(Animation::with(9055, 0, Some(Priority::High))),
            )?;
        } else {
            let d = ctx.npc(id)?;
            let player = owner.as_ref().ok_or(Error::Null)?;
            let size = d.read().size;
            ctx.size(player, size)?;
            for (index, slot) in [
                (0, CombatAnimation::Attack),
                (3, CombatAnimation::Defence),
                (4, CombatAnimation::Death),
            ] {
                let present = d
                    .read()
                    .combat_animation(index)
                    .map_err(|e| match e {
                        npc::Error::Type => Error::Type,
                        npc::Error::Null => Error::Null,
                        _ => Error::Binding(e.to_string()),
                    })?
                    .is_some();
                if present {
                    let properties = ctx.properties(player)?;
                    let animation = d
                        .read()
                        .combat_animation(index)
                        .map_err(|e| match e {
                            npc::Error::Type => Error::Type,
                            npc::Error::Null => Error::Null,
                            _ => Error::Binding(e.to_string()),
                        })?
                        .map(Animation::from_shared);
                    ctx.combat_animation(properties, slot, animation)?;
                }
            }
        }
        self.sync(ctx)
    }
    pub fn prepare_body(
        &self,
        ctx: &mut impl BodyContext,
        player: Option<&NodeHandle>,
    ) -> Result<()> {
        let player = player.ok_or(Error::Null)?;
        let render = ctx.render(player)?;
        if ctx.prepared(render)? {
            return Ok(());
        }
        let render = ctx.render(player)?;
        ctx.set_prepared(render, true)?;
        let mut get = |slot| -> Result<Option<ItemHandle>> { Ok(ctx.equipment(player)?.get(slot)) };
        let chest = get(4)?;
        let shield = get(5)?;
        let legs = get(7)?;
        let hat = get(0)?;
        let hands = get(9)?;
        let feet = get(10)?;
        let cape = get(1)?;
        let amulet = get(2)?;
        let weapon = get(3)?;
        let hood = cape
            .as_ref()
            .is_some_and(|i| matches!(i.read().id(), 4041 | 4042));
        for (slot, item) in [(0, hat.as_ref()), (1, cape.as_ref()), (2, amulet.as_ref())] {
            if item.is_some() {
                self.draw_item(slot, item)?;
            } else {
                self.clear_body_part(slot)?;
            }
        }
        if !self.riding() {
            for (slot, item) in [(3, weapon.as_ref()), (5, shield.as_ref())] {
                if item.is_some() {
                    self.draw_item(slot, item)?;
                } else {
                    self.clear_body_part(slot)?;
                }
            }
        } else {
            self.clear_body_part(5)?;
            self.draw_clothes(3, 82)?;
        }
        if chest.is_some() {
            self.draw_item(4, chest.as_ref())?;
        } else {
            self.draw_clothes(4, self.look(2)?)?;
        }
        if chest
            .as_ref()
            .map(|i| flag(i, "remove_sleeves"))
            .transpose()?
            .unwrap_or(false)
        {
            self.clear_body_part(6)?;
        } else {
            self.draw_clothes(6, self.look(3)?)?;
        }
        if legs.is_some() {
            self.draw_item(7, legs.as_ref())?;
        } else {
            self.draw_clothes(7, self.look(5)?)?;
        }
        if hat
            .as_ref()
            .map(|i| flag(i, "remove_head"))
            .transpose()?
            .unwrap_or(false)
            || hood
        {
            self.clear_body_part(8)?;
        } else {
            self.draw_clothes(8, self.look(0)?)?;
        }
        if hat
            .as_ref()
            .map(|i| flag(i, "hat"))
            .transpose()?
            .unwrap_or(false)
        {
            self.hat_clipping()?;
        }
        if hands.is_some() {
            self.draw_item(9, hands.as_ref())?;
        } else {
            self.draw_clothes(9, self.look(4)?)?;
        }
        if feet.is_some() {
            self.draw_item(10, feet.as_ref())?;
        } else {
            self.draw_clothes(10, self.look(6)?)?;
        }
        if hat
            .as_ref()
            .map(|i| flag(i, "remove_beard"))
            .transpose()?
            .unwrap_or(false)
        {
            self.clear_body_part(11)?;
        } else {
            self.draw_clothes(11, self.look(1)?)?;
        }
        Ok(())
    }
    pub fn ride_cart(&self, ctx: &mut impl Context, ride: bool) -> Result<()> {
        if !ride {
            self.set_animations(ctx)?;
        } else {
            let player = self.player().ok_or(Error::Null)?;
            let appearance = ctx.appearance(&player)?;
            appearance.set_render_animation(ctx, Some(&Animation::new(211)))?;
        }
        let player = self.player().ok_or(Error::Null)?;
        ctx.appearance(&player)?.set_riding(ride);
        let appearance = ctx.appearance(&player)?;
        appearance.sync(ctx)
    }
    pub fn fly_copter(&self, ctx: &mut impl Context, definitions: &Registry) -> Result<()> {
        let player = self.player().ok_or(Error::Null)?;
        ctx.animate(&player, Animation::new(8956))?;
        let mut equipment = ctx.equipment(&player)?;
        let item = Shared::new(Item::new(12842, 1, 1000, definitions));
        equipment.replace(Some(&item), 3, true, definitions, ctx)?;
        for (slot, id) in [
            (0, 8964),
            (2, 8961),
            (6, 8963),
            (3, 8963),
            (5, 8963),
            (4, 8963),
            (1, 8963),
        ] {
            ctx.appearance(&player)?.set_animation(slot, id)?;
        }
        Ok(())
    }
}
