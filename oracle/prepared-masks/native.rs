use rs09_server::{
    definitions::item::Shared,
    world::{
        appearance,
        doors::Location,
        npc_render::{Handle as NpcHandle, Npc},
        player_render::{Handle, Player},
        player_walk::{Node, State},
        prepared_masks::{FaceTarget, PreparedMasks},
    },
};
use serde_json::{Value, json};
fn number(v: &Value, k: &str) -> i32 {
    v[k].as_i64().unwrap_or(0) as i32
}
struct Harness {
    masks: PreparedMasks,
    players: [Handle; 2],
    npc: NpcHandle,
    world: State,
    nodes: [Node; 3],
    locations: [Location; 2],
    buffer: Vec<u8>,
}
impl Harness {
    fn new() -> Self {
        let locations = [Location::new(3207, 3216, 0), Location::new(3208, 3216, 0)];
        let players = [
            Shared::new(Player::new(1, locations[0].clone(), 2000)),
            Shared::new(Player::new(2, locations[0].clone(), 2000)),
        ];
        let npc = Shared::new(Npc::new(3, 1, locations[0].clone()));
        let mut world = State::default();
        let nodes = [
            world.add(1, locations[0].clone(), [1, 2, 3, 4]),
            world.add(2, locations[0].clone(), [1, 2, 3, 4]),
            world.add_npc(3, locations[0].clone(), [1, 2, 3, 4]),
        ];
        let masks =
            PreparedMasks::new(players[0].clone(), Shared::new(appearance::Data::default()));
        Self {
            masks,
            players,
            npc,
            world,
            nodes,
            locations,
            buffer: vec![],
        }
    }
    fn act(&mut self, a: &Value) -> Value {
        match a["op"].as_str().unwrap() {
            "location" => json!(
                self.masks
                    .face_location(
                        a["location"]
                            .as_u64()
                            .map(|i| self.locations[i as usize].clone())
                    )
                    .unwrap()
            ),
            "face" => {
                let target = a["target"].as_u64().map(|i| {
                    let node = self.world.retain_node(self.nodes[i as usize]).unwrap();
                    if i == 1 {
                        FaceTarget::Player(node, self.players[1].clone())
                    } else {
                        FaceTarget::Npc(node, self.npc.clone())
                    }
                });
                json!(self.masks.face_entity(target).unwrap())
            }
            "move" => {
                let c = a["coords"].as_array().unwrap();
                self.locations[number(a, "location") as usize].set(
                    c[0].as_i64().unwrap() as i32,
                    c[1].as_i64().unwrap() as i32,
                    c[2].as_i64().unwrap() as i32,
                );
                Value::Null
            }
            "index" => {
                let i = number(a, "entity") as usize;
                if i < 2 {
                    self.players[i].write().index = number(a, "value")
                } else {
                    self.npc.write().index = number(a, "value")
                };
                Value::Null
            }
            "animate" => json!(
                self.masks
                    .animate(number(a, "id"), number(a, "delay"))
                    .unwrap()
            ),
            "spot" => json!(
                self.masks
                    .spot(appearance::SpotAnimation {
                        id: number(a, "id"),
                        height: number(a, "height"),
                        delay: number(a, "delay")
                    })
                    .unwrap()
            ),
            "prepare" => {
                self.masks.prepare().unwrap();
                Value::Null
            }
            "reset" => {
                self.masks.reset();
                Value::Null
            }
            "write" => {
                self.masks
                    .write(
                        self.players[0].clone(),
                        &mut self.buffer,
                        a["appearance"].as_bool().unwrap_or(true),
                        a["synced"].as_bool().unwrap_or(false),
                    )
                    .unwrap();
                Value::Null
            }
            op => panic!("unknown {op}"),
        }
    }
    fn snapshot(&self, result: Value) -> Value {
        let s = self.masks.status();
        let visible = self.players[0].read().masks;
        assert_eq!(
            (visible.required, visible.synced, visible.appearance_stamp),
            (s.update_required(), s.has_synced(), s.appearance_stamp)
        );
        json!({"result":result,"updating":s.updating,"required":s.update_required(),"synced":s.has_synced(),"stamp":s.appearance_stamp,
   "presence":s.presence,"syncedPresence":s.synced_presence,"bytes":self.buffer,"indices":[self.players[0].read().index,self.players[1].read().index,self.npc.read().index],
   "locations":self.locations.iter().map(Location::coordinates).collect::<Vec<_>>()})
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut h = Harness::new();
    Value::Array(
        input["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                let r = h.act(a);
                h.snapshot(r)
            })
            .collect(),
    )
}
