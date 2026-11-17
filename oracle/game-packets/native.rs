//! Test-only JSON observations of public native actions and original wire output.
use rs09_protocol::{
    buffer::Reader,
    game::{
        ClientAction, Direction, Frame,
        updates::{self, ScriptArg, WireItem},
    },
};
use serde_json::{Value, json};
fn action_json(action: ClientAction) -> Value {
    use ClientAction::*;
    match action {
        ItemAction {
            opt_index,
            item_id,
            slot,
            iface,
            child,
        } => {
            json!({"type":"ItemAction", "optIndex":opt_index, "itemId":item_id, "slot":slot, "iface":iface, "child":child})
        }
        NpcAction {
            opt_index,
            npc_index,
        } => json!({"type":"NpcAction", "optIndex":opt_index, "npcIndex":npc_index}),
        SceneryAction {
            opt_index,
            id,
            x,
            y,
        } => json!({"type":"SceneryAction", "optIndex":opt_index, "id":id, "x":x, "y":y}),
        PlayerAction {
            opt_index,
            other_index,
        } => json!({"type":"PlayerAction", "optIndex":opt_index, "otherIndex":other_index}),
        GroundItemAction {
            opt_index,
            id,
            x,
            y,
        } => json!({"type":"GroundItemAction", "optIndex":opt_index, "id":id, "x":x, "y":y}),
        UseWithNpc {
            item_id,
            npc_index,
            iface,
            slot,
        } => {
            json!({"type":"UseWithNpc", "itemId":item_id, "npcIndex":npc_index, "iface":iface, "slot":slot})
        }
        UseWithPlayer {
            item_id,
            other_index,
            iface,
            slot,
        } => {
            json!({"type":"UseWithPlayer", "itemId":item_id, "otherIndex":other_index, "iface":iface, "slot":slot})
        }
        UseWithItem {
            used_id,
            used_with_id,
            used_slot,
            used_with_slot,
            used_iface,
            used_with_iface,
            used_child,
            used_with_child,
        } => {
            json!({"type":"UseWithItem", "usedId":used_id, "usedWithId":used_with_id, "usedSlot":used_slot, "usedWithSlot":used_with_slot, "usedIface":used_iface, "usedWithIface":used_with_iface, "usedChild":used_child, "usedWithChild":used_with_child})
        }
        UseWithScenery {
            item_id,
            slot,
            scenery_id,
            x,
            y,
        } => {
            json!({"type":"UseWithScenery", "itemId":item_id, "slot":slot, "sceneryId":scenery_id, "x":x, "y":y})
        }
        UseWithGroundItem {
            used_id,
            with_id,
            iface,
            child,
            slot,
            x,
            y,
        } => {
            json!({"type":"UseWithGroundItem", "usedId":used_id, "withId":with_id, "iface":iface, "child":child, "slot":slot, "x":x, "y":y})
        }
        IfAction {
            opcode,
            opt_index,
            iface,
            child,
            slot,
            item_id,
        } => {
            json!({"type":"IfAction", "opcode":opcode, "optIndex":opt_index, "iface":iface, "child":child, "slot":slot, "itemId":item_id})
        }
        ContinueOption {
            iface,
            child,
            slot,
            opcode,
        } => {
            json!({"type":"ContinueOption", "iface":iface, "child":child, "slot":slot, "opcode":opcode})
        }
        CloseIface => json!({"type":"CloseIface"}),
        ComponentGroundItemAction {
            iface,
            child,
            slot,
            item_id,
            x,
            y,
        } => {
            json!({"type":"ComponentGroundItemAction", "iface":iface, "child":child, "slot":slot, "itemId":item_id, "x":x, "y":y})
        }
        ComponentPlayerAction {
            other_index,
            iface,
            child,
        } => {
            json!({"type":"ComponentPlayerAction", "otherIndex":other_index, "iface":iface, "child":child})
        }
        ComponentSceneryAction {
            iface,
            child,
            scenery_id,
            x,
            y,
        } => {
            json!({"type":"ComponentSceneryAction", "iface":iface, "child":child, "sceneryId":scenery_id, "x":x, "y":y})
        }
        ComponentNpcAction {
            iface,
            child,
            npc_index,
        } => {
            json!({"type":"ComponentNpcAction", "iface":iface, "child":child, "npcIndex":npc_index})
        }
        ComponentItemAction {
            iface,
            child,
            item_id,
            slot,
        } => {
            json!({"type":"ComponentItemAction", "iface":iface, "child":child, "itemId":item_id, "slot":slot})
        }
        SlotSwitchMultiComponent {
            source_iface,
            source_child,
            source_slot,
            dest_iface,
            dest_child,
            dest_slot,
        } => {
            json!({"type":"SlotSwitchMultiComponent", "sourceIface":source_iface, "sourceChild":source_child, "sourceSlot":source_slot, "destIface":dest_iface, "destChild":dest_child, "destSlot":dest_slot})
        }
        SlotSwitchSingleComponent {
            iface,
            child,
            source_slot,
            dest_slot,
            is_insert,
        } => {
            json!({"type":"SlotSwitchSingleComponent", "iface":iface, "child":child, "sourceSlot":source_slot, "destSlot":dest_slot, "isInsert":is_insert})
        }
        SceneryExamine { id } => json!({"type":"SceneryExamine", "id":id}),
        ItemExamine { id } => json!({"type":"ItemExamine", "id":id}),
        NpcExamine { id } => json!({"type":"NpcExamine", "id":id}),
        WorldspaceWalk {
            dest_x,
            dest_y,
            is_run,
        } => json!({"type":"WorldspaceWalk", "destX":dest_x, "destY":dest_y, "isRun":is_run}),
        InteractWalk {
            dest_x,
            dest_y,
            is_run,
        } => json!({"type":"InteractWalk", "destX":dest_x, "destY":dest_y, "isRun":is_run}),
        MinimapWalk {
            dest_x,
            dest_y,
            clicked_x,
            clicked_y,
            rotation,
            is_run,
        } => {
            json!({"type":"MinimapWalk", "destX":dest_x, "destY":dest_y, "clickedX":clicked_x, "clickedY":clicked_y, "rotation":rotation, "isRun":is_run})
        }
        TrackingFocus { is_focused } => json!({"type":"TrackingFocus", "isFocused":is_focused}),
        TrackingCameraPos { x, y } => json!({"type":"TrackingCameraPos", "x":x, "y":y}),
        TrackingDisplayUpdate {
            window_mode,
            screen_width,
            screen_height,
            display_mode,
        } => {
            json!({"type":"TrackingDisplayUpdate", "windowMode":window_mode, "screenWidth":screen_width, "screenHeight":screen_height, "displayMode":display_mode})
        }
        TrackingAfkTimeout => json!({"type":"TrackingAfkTimeout"}),
        TrackingMouseClick {
            x,
            y,
            is_right_click,
            delay,
        } => {
            json!({"type":"TrackingMouseClick", "x":x, "y":y, "isRightClick":is_right_click, "delay":delay})
        }
        InputIntResponse { response } => json!({"type":"InputPromptResponse", "response":response}),
        InputStringResponse { response } => {
            json!({"type":"InputPromptResponse", "response":response})
        }
        GESetOfferItem { item_id } => json!({"type":"GESetOfferItem", "itemId":item_id}),
        ChatSetting {
            public,
            private,
            trade,
        } => json!({"type":"ChatSetting", "public":public, "private":private, "trade":trade}),
        TrackFinished { track_id } => json!({"type":"TrackFinished", "trackId":track_id}),
        PacketCountUpdate { count } => json!({"type":"PacketCountUpdate", "count":count}),
        Ping => json!({"type":"Ping"}),
        NoProcess => json!({"type":"NoProcess"}),
        PlayerPrefsUpdate { prefs } => json!({"type":"PlayerPrefsUpdate", "prefs":prefs}),
    }
}
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array().unwrap().iter().map(|v| n(v) as u8).collect()
}
pub fn evaluate(c: &Value) -> Value {
    let kind = c["kind"].as_str().unwrap();
    if kind == "readUpdate" {
        return update_json(&Frame {
            opcode: n(&c["opcode"]) as u8,
            payload: bytes(&c["data"]),
        });
    }
    if kind == "decode" {
        let data = bytes(&c["data"]);
        let mut r = Reader::new(&data);
        return match ClientAction::decode(n(&c["opcode"]) as u8, &mut r) {
            Ok(Some(a)) => json!({"position":r.position(),"action":action_json(a)}),
            Ok(None) => panic!("unsupported fixture opcode"),
            Err(_) => json!({"position":r.position(),"error":"underflow"}),
        };
    }
    let a = |i: usize| c["args"].get(i).map_or(0, n);
    let (x, y, counter, d, e) = (a(0), a(1), a(2), a(3), a(4));
    let mut count = counter;
    let f: Frame = match kind {
        "skill" => updates::skill_level(x, y, c["value"].as_str().unwrap().parse().unwrap()),
        "energy" => updates::run_energy(c["value"].as_str().unwrap().parse().unwrap()),
        "varp" => updates::varp(x, y),
        "varbit" => updates::varbit(x, y),
        "varc" => {
            count = count.wrapping_add(1);
            updates::varc(x, y, counter)
        }
        "events" => {
            count = count.wrapping_add(1);
            updates::interface_events(x, y, d, e, counter)
        }
        "script" => {
            count = count.wrapping_add(1);
            let objects: Vec<_> = c["objects"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| {
                    if let Some(t) = a["text"].as_str() {
                        ScriptArg::Text(t.as_bytes().to_vec())
                    } else {
                        ScriptArg::Int(n(&a["integer"]))
                    }
                })
                .collect();
            updates::run_client_script(
                x,
                c["types"].as_str().unwrap().as_bytes(),
                &objects,
                counter,
            )
            .unwrap()
        }
        "locAdd" => updates::loc_add(x & 7, y & 7, counter, d, e),
        "locDel" => updates::loc_del(x & 7, y & 7, counter, d),
        "origin" => updates::zone_origin(x, y, false),
        "enclosed" => updates::zone_enclosed(
            x,
            y,
            &[
                updates::loc_add(x & 7, y & 7, counter, d, e),
                updates::loc_del(x & 7, y & 7, counter, d),
            ],
        )
        .unwrap(),
        "open" => {
            count = count.wrapping_add(1);
            updates::open_interface(i32::from((d & 1) != 0), x, counter, y)
        }
        "pane" => {
            count = count.wrapping_add(1);
            updates::windows_pane(x, y, counter)
        }
        "close" => {
            count = count.wrapping_add(1);
            updates::close_interface(counter, x)
        }
        "hidden" => {
            count = count.wrapping_add(1);
            updates::interface_hidden((y & 1) != 0, counter, x)
        }
        "text" => {
            count = count.wrapping_add(1);
            updates::interface_text(x, c["text"].as_str().unwrap().as_bytes(), counter).unwrap()
        }
        "clear" => updates::inventory_clear(x),
        "full" | "partial" => {
            let items: Vec<_> = c["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| {
                    if i.is_null() {
                        None
                    } else {
                        Some(WireItem {
                            id: n(&i["id"]),
                            amount: n(&i["amount"]),
                        })
                    }
                })
                .collect();
            if kind == "full" {
                updates::inventory_full(x, y, &items).unwrap()
            } else {
                let slots: Vec<_> = c["slots"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| {
                        let s = n(s);
                        (s, items[s as usize])
                    })
                    .collect();
                updates::inventory_partial(x, y, &slots).unwrap()
            }
        }
        _ => panic!("unknown packet request {kind}"),
    };
    let mut wire = Vec::new();
    f.write(Direction::ServerToClient, None, &mut wire).unwrap();
    json!({"wire":wire,"counter":count,"fields":update_json(&f)})
}
fn item_json(i: WireItem) -> Value {
    json!({"id":i.id,"amount":i.amount})
}
fn update_json(f: &Frame) -> Value {
    use updates::ServerUpdate::*;
    let update = match updates::ServerUpdate::decode(f) {
        Ok(Some(u)) => u,
        Err(_) => return json!({"error":"malformed"}),
        Ok(None) => panic!("unported update"),
    };
    match update {
        // Decoded by the client since this corpus was recorded; no scenario
        // in it sends either message.
        GameMessage(text) => json!({"type":"GameMessage","text":text}),
        ChatSettings(values) => json!({"type":"ChatSettings","values":values}),
        SynthSound { id, loops, delay } => {
            json!({"type":"SynthSound","id":id,"loops":loops,"delay":delay})
        }
        Skill {
            skill,
            boosted,
            experience,
        } => json!({"type":"Skill","skill":skill,"boosted":boosted,"experience":experience}),
        RunEnergy { energy } => json!({"type":"RunEnergy","energy":energy}),
        RunWeight { weight } => json!({"type":"RunWeight","weight":weight}),
        Varp { id, value } => json!({"type":"Varp","id":id,"value":value}),
        Varbit { id, value } => json!({"type":"Varbit","id":id,"value":value}),
        Varc { counter, id, value } => {
            json!({"type":"Varc","counter":counter,"id":id,"value":value})
        }
        InterfaceEvents {
            counter,
            pointer,
            start,
            end,
            mask,
        } => {
            json!({"type":"InterfaceEvents","counter":counter,"pointer":pointer,"start":start,"end":end,"mask":mask})
        }
        RunClientScript {
            counter,
            id,
            types,
            arguments,
        } => {
            json!({"type":"RunClientScript","counter":counter,"id":id,"types":types,"arguments":arguments.into_iter().map(|a|match a{ScriptArg::Int(v)=>json!({"type":"Int","value":v}),ScriptArg::Text(v)=>json!({"type":"Text","value":v})}).collect::<Vec<_>>()})
        }
        GroundAdd {
            x,
            y,
            id,
            amount,
            exclude_player,
        } => {
            json!({"type":"GroundAdd","x":x,"y":y,"id":id,"amount":amount,"exclude_player":exclude_player})
        }
        GroundRemove { x, y, id } => json!({"type":"GroundRemove","x":x,"y":y,"id":id}),
        GroundAmount {
            x,
            y,
            id,
            old_amount,
            amount,
        } => {
            json!({"type":"GroundAmount","x":x,"y":y,"id":id,"old_amount":old_amount,"amount":amount})
        }
        ZoneOrigin { full, x, y } => json!({"type":"ZoneOrigin","full":full,"x":x,"y":y}),
        ZoneEnclosed { x, y, frames } => {
            json!({"type":"ZoneEnclosed","x":x,"y":y,"frames":frames.into_iter().map(|f|json!({"opcode":f.opcode,"payload":f.payload})).collect::<Vec<_>>()})
        }
        LocAdd {
            x,
            y,
            shape,
            rotation,
            id,
        } => json!({"type":"LocAdd","x":x,"y":y,"shape":shape,"rotation":rotation,"id":id}),
        LocDel {
            x,
            y,
            shape,
            rotation,
        } => json!({"type":"LocDel","x":x,"y":y,"shape":shape,"rotation":rotation}),
        LocAnim {
            x,
            y,
            shape,
            rotation,
            sequence,
        } => {
            json!({"type":"LocAnim","x":x,"y":y,"shape":shape,"rotation":rotation,"sequence":sequence})
        }
        OpenInterface {
            kind,
            pointer,
            counter,
            interface,
        } => {
            json!({"type":"OpenInterface","kind":kind,"pointer":pointer,"counter":counter,"interface":interface})
        }
        WindowsPane {
            interface,
            reset,
            counter,
        } => json!({"type":"WindowsPane","interface":interface,"reset":reset,"counter":counter}),
        CloseInterface { counter, pointer } => {
            json!({"type":"CloseInterface","counter":counter,"pointer":pointer})
        }
        InterfaceHidden {
            hidden,
            counter,
            pointer,
        } => json!({"type":"InterfaceHidden","hidden":hidden,"counter":counter,"pointer":pointer}),
        InterfaceText {
            pointer,
            text,
            counter,
        } => json!({"type":"InterfaceText","pointer":pointer,"text":text,"counter":counter}),
        InterfaceAnimation {
            pointer,
            sequence,
            counter,
        } => {
            json!({"type":"InterfaceAnimation","pointer":pointer,"sequence":sequence,"counter":counter})
        }
        InterfaceModel {
            pointer,
            model,
            counter,
        } => json!({"type":"InterfaceModel","pointer":pointer,"model":model,"counter":counter}),
        InterfaceNpcHead {
            pointer,
            npc,
            counter,
        } => json!({"type":"InterfaceNpcHead","pointer":pointer,"npc":npc,"counter":counter}),
        InterfacePlayerHead { pointer, counter } => {
            json!({"type":"InterfacePlayerHead","pointer":pointer,"counter":counter})
        }
        InterfaceAngle {
            pointer,
            pitch,
            yaw,
            zoom,
            counter,
        } => {
            json!({"type":"InterfaceAngle","pointer":pointer,"pitch":pitch,"yaw":yaw,"zoom":zoom,"counter":counter})
        }
        InterfaceObject {
            pointer,
            item,
            value,
            counter,
        } => {
            json!({"type":"InterfaceObject","pointer":pointer,"item":item,"value":value,"counter":counter})
        }
        InterfacePosition {
            pointer,
            x,
            y,
            counter,
        } => json!({"type":"InterfacePosition","pointer":pointer,"x":x,"y":y,"counter":counter}),
        InventoryClear { pointer } => json!({"type":"InventoryClear","pointer":pointer}),
        InventoryFull {
            pointer,
            container,
            items,
        } => {
            json!({"type":"InventoryFull","pointer":pointer,"container":container,"items":items.into_iter().map(item_json).collect::<Vec<_>>()})
        }
        InventoryPartial {
            pointer,
            container,
            slots,
        } => {
            json!({"type":"InventoryPartial","pointer":pointer,"container":container,"slots":slots.into_iter().map(|s|json!({"slot":s.slot,"item":item_json(s.item)})).collect::<Vec<_>>()})
        }
    }
}
