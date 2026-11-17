//! A finite data-driven Open/Close agent. It reacts to actual scenery updates;
//! there is no renderer, cache-side fake replacement or success-on-send flag.
use rs09_headless::{
    protocol::game::{players, rebuild},
    scenery::{Event, Observer},
    session::{SceneryAction, Session},
};
use serde_json::json;
use std::{collections::BTreeMap, io, time::Duration};
use tokio::time::timeout;
#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let address = option("--address")
        .map_or("127.0.0.1:43632", String::as_str)
        .parse()?;
    let username = option("--username").map_or("dataagent", String::as_str);
    let websocket = args.iter().any(|a| a == "--websocket");
    let closed = option("--closed-id")
        .map_or("12348", String::as_str)
        .parse::<i32>()?;
    let opened = option("--opened-id")
        .map_or("10327", String::as_str)
        .parse::<i32>()?;
    let tile = option("--tile")
        .map_or("3207,3217", String::as_str)
        .split(',')
        .map(str::parse::<i32>)
        .collect::<Result<Vec<_>, _>>()?;
    let tile: [i32; 2] = tile.try_into().map_err(|_| "--tile needs X,Y")?;
    let mut session = Session::connect(address, websocket, username, "testing")
        .await?
        .into_managed();
    let sender = session.sender();
    let mut base: Option<[i32; 2]> = None;
    let mut players = players::State::new(0, 0);
    let mut observer = Observer::default();
    let mut player_updates = 0;
    let mut opening_sent = false;
    let mut closing_sent = false;
    let mut opened_observed = false;
    let mut restored_observed = false;
    let mut requests = vec![];
    let mut events = vec![];
    let mut sounds = vec![];
    let mut counts = BTreeMap::<u8, usize>::new();
    timeout(Duration::from_secs(20),async{
        while !restored_observed{
            let frame=session.next_frame().await?;*counts.entry(frame.opcode).or_default()+=1;
            match frame.opcode{
                162=>{
                    let next=rebuild::decode_normal(&frame).map_err(io::Error::other)?;
                    if let Some(old)=base{let new=next.base();players.rebase([new[0]-old[0],new[1]-old[1]]);}
                    base=Some(next.base());players.plane=next.plane;players.self_player_mut().teleport(next.local_x,next.local_y,false);
                }
                225=>{players.decode_frame(&frame,player_updates as i32).map_err(io::Error::other)?;player_updates+=1;
                    if !opening_sent&&base.is_some(){
                        let action=SceneryAction{option:0,id:closed,tile};let wire=sender.scenery(action).await?;
                        requests.push(json!({"phase":"open","id":closed,"tile":tile,"wire":wire}));opening_sent=true;
                    }
                }
                86=>return Err(io::Error::other("server logged out")),
                _=>{}
            }
            observer.frame(&frame,players.plane,base,player_updates).map_err(io::Error::other)?;
            for observation in observer.drain(){
                match observation.event{
                    Event::Location{slot,location,..}=>{
                        events.push(json!({"plane":slot.plane,"tile":slot.tile,"id":location.map(|l|l.id),"playerUpdate":observation.player_update}));
                        if opening_sent&&location.is_some_and(|l|l.id==opened)&&!closing_sent{
                            opened_observed=true;
                            // The server supplies the replacement's actual tile.
                            let action=SceneryAction{option:0,id:opened,tile:slot.tile};let wire=sender.scenery(action).await?;
                            requests.push(json!({"phase":"close","id":opened,"tile":slot.tile,"wire":wire}));closing_sent=true;
                        }else if closing_sent&&location.is_some_and(|l|l.id==closed)&&slot.tile==tile{restored_observed=true;}
                    }
                    Event::Sound{id,loops,delay}=>sounds.push(json!({"id":id,"loops":loops,"delay":delay})),
                    Event::ResetChunk{..}=>{}
                }
            }
        }Ok::<(),io::Error>(())
    }).await.map_err(|_|"agent did not observe the full round trip within20seconds")??;
    session.close().await?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"scope":"finite data-only scenery agent; sent requests and authoritative observations, not a capacity or complete-game claim","username":username,"websocket":websocket,"playerUpdates":player_updates,"openedObserved":opened_observed,"restoredObserved":restored_observed,"requests":requests,"sceneryEvents":events,"sounds":sounds,"frameCounts":counts})
        )?
    );
    Ok(())
}
