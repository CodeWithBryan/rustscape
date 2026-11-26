//! CS2 commands 5200..=5220: the world map (`ScriptRunner` 4477-4647).
//!
//! Each arm is a transcription of the original branch for that opcode; the
//! stack order follows its `isp`/`ssp` arithmetic. The map itself is
//! `state.world_map` (see `world_map_host.rs`). Before any map list is
//! installed, which is always the case in the browser build, the commands
//! answer from the Java's initial statics: empty strings, zeros, zoom 200,
//! default zoom -1, not loaded.
//!
//! Where the Java throws, the command fails in the same place and the script
//! stops as the original's does: `panToCoords` (5214, and 5203/5219 once a
//! label is found) dereferences `WorldMap.component`, which is null unless
//! the interface pass met the map component in this cycle.
use super::{Result, Stack, Vm};
use crate::ui::state::State;

/// `int >> 14 & 0x3FFF` and `int & 0x3FFF`: the packed display coordinate of
/// commands 5206, 5214 and 5215.
fn coordinate(packed: i32) -> [i32; 2] {
    [packed >> 14 & 0x3fff, packed & 0x3fff]
}

const NO_COMPONENT: &str = "World map component is absent";

impl Vm {
    pub(super) fn world_map_operation(
        &mut self,
        state: &mut State,
        op: i32,
        s: &mut Stack,
    ) -> Result<()> {
        let map = &mut state.world_map.map;
        match op {
            5200 => {
                let level = s.pop()?;
                map.set_target_zoom(level);
            }
            5201 => s.ints.push(map.get_target_zoom()),
            5202 => {
                let function = s.pop()?;
                map.highlight_map_element(function);
            }
            5203 | 5219 => {
                let text = s.text()?;
                let index = if op == 5203 {
                    map.find_label_by_prefix(&text)
                } else {
                    map.find_label_by_exact_name(&text)
                };
                let target = map.labels.as_ref().and_then(|labels| {
                    let at = usize::try_from(index).ok()?;
                    Some([*labels.coord_x.get(at)?, *labels.coord_y.get(at)?])
                });
                if let Some([x, y]) = target
                    && !map.pan_to_coords(i32::from(x), i32::from(y))
                {
                    return Err(NO_COMPONENT.into());
                }
            }
            5204 => {
                let prefix = s.text()?;
                s.strings.push(Some(map.get_label_text_by_prefix(&prefix)));
            }
            5205 => {
                let name = s.text()?;
                map.switch_map(&name);
            }
            5206 => {
                let [x, y] = coordinate(s.pop()?);
                s.strings.push(Some(
                    map.maps
                        .get_containing_source(x, y)
                        .map_or_else(String::new, |source| source.group.clone()),
                ));
            }
            5207 => {
                let group = s.text()?;
                s.strings.push(Some(
                    map.maps
                        .get(&group)
                        .map_or_else(String::new, |found| found.name.clone()),
                ));
            }
            5208 => s.ints.extend(map.viewport_size()),
            5209 => s.ints.extend(map.view_coord()),
            5210 => s.ints.extend(map.current_map_center()),
            5211 => s.ints.extend(map.current_map_size()),
            5212 | 5213 => {
                let index = if op == 5212 {
                    map.get_first_visible_label()
                } else {
                    map.get_next_visible_label()
                };
                // `names[index].replaceAll(SPACE, BR_TAG)`: JagString's first
                // argument is the replacement, so `<br>` becomes a space.
                s.strings.push(Some(map.label_text(index)));
                s.ints.push(map.label_size(index));
            }
            5214 => {
                let [x, y] = coordinate(s.pop()?);
                if !map.pan_to_coords(x, y) {
                    return Err(NO_COMPONENT.into());
                }
            }
            5215 => {
                let [x, y] = coordinate(s.pop()?);
                let group = s.text()?;
                s.ints.push(i32::from(map.maps.contains_at(x, y, &group)));
            }
            5216 => {
                let index = s.pop()?;
                map.maps.toggle_map_visibility(index);
            }
            5217 => {
                let index = s.pop()?;
                s.ints.push(i32::from(map.maps.is_map_visible(index)));
            }
            5218 => s.ints.push(map.current_map_default_zoom()),
            5220 => s.ints.push(i32::from(map.is_loaded())),
            _ => return Err(format!("Unsupported CS2 opcode {op}")),
        }
        Ok(())
    }

    /// Sends what the world map queued for the server: for each staff
    /// Ctrl-click, `Cheat.teleport(x, y, 0)` then `ClientProt.closeWidget()`;
    /// and one NO_TIMEOUT (opcode 93) for each `ClientProt.ping(true)` of a
    /// load step.
    ///
    /// `Cheat.execute` ends in `sendCheatPacket`: CLIENT_CHEAT (44) carrying
    /// the line after its `::`, which is also what `chat::public` sends for a
    /// typed command. None of the client-side commands `execute` checks
    /// first matches a `::tele` line.
    pub fn world_map_requests(&mut self, state: &mut State) {
        for [x, y] in std::mem::take(&mut state.world_map.teleports) {
            let command = rs09_protocol::game::outgoing::teleport_command(x, y, 0);
            let mut w = rs09_protocol::buffer::Writer::new();
            match w.pjstr(&command.as_bytes()[2..]) {
                Ok(()) => self.outgoing.push(rs09_protocol::game::Frame {
                    opcode: 44,
                    payload: w.into_vec(),
                }),
                Err(error) => state.diagnostic(format!("World map teleport: {error}")),
            }
            self.close_modal(state);
        }
        for _ in 0..std::mem::take(&mut state.world_map.pings) {
            self.outgoing.push(rs09_protocol::game::Frame {
                opcode: 93,
                payload: Vec::new(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::Pack;

    fn setup() -> (State, Vm) {
        let manifest = br#"{"source":"world map","files":{}}"#;
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        (State::new(Pack::decode(bytes).unwrap()), Vm::default())
    }

    fn run(
        vm: &mut Vm,
        state: &mut State,
        op: i32,
        ints: &[i32],
        strings: &[&str],
    ) -> Result<(Vec<i32>, Vec<String>)> {
        let mut s = Stack {
            ints: ints.to_vec(),
            strings: strings.iter().map(|t| Some((*t).to_owned())).collect(),
        };
        vm.extended_operation(state, op, &mut s)?;
        Ok((s.ints, s.strings.into_iter().map(Option::unwrap).collect()))
    }

    /// Transcription test: with no map list (every browser session, and a
    /// native one without a cache) each command leaves what the Java statics
    /// give before `MapList.init`, and none of the queries fails.
    #[test]
    fn without_a_map_every_command_answers_from_the_initial_statics() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        let ok = |vm: &mut Vm, st: &mut State, op, ints: &[i32], strings: &[&str]| {
            run(vm, st, op, ints, strings).unwrap_or_else(|e| panic!("opcode {op}: {e}"))
        };
        let none: (Vec<i32>, Vec<String>) = (vec![], vec![]);
        let text = |value: &str| (vec![], vec![value.to_owned()]);
        // 5200 consumes its level; 5201 reads `targetZoom == 0.0F` as 200.
        assert_eq!(ok(vm, st, 5200, &[9, 37], &[]), (vec![9], vec![]));
        assert_eq!(st.world_map.map.target_zoom, 3.0);
        assert_eq!(ok(vm, st, 5201, &[], &[]), (vec![37], vec![]));
        st.world_map.map.target_zoom = 0.0;
        assert_eq!(ok(vm, st, 5201, &[], &[]), (vec![200], vec![]));
        assert_eq!(ok(vm, st, 5202, &[7], &[]), none);
        assert_eq!(
            [
                st.world_map.map.highlighted_map_function,
                st.world_map.map.highlight_pulse_count,
                st.world_map.map.highlight_pulse_counter
            ],
            [7, 3, 20]
        );
        // No labels: nothing to pan to, and the text is empty.
        assert_eq!(ok(vm, st, 5203, &[], &["lum"]), none);
        assert_eq!(ok(vm, st, 5219, &[], &["Lumbridge"]), none);
        assert_eq!(ok(vm, st, 5204, &[], &["keep", "lum"]), {
            (vec![], vec!["keep".to_owned(), String::new()])
        });
        assert_eq!(ok(vm, st, 5205, &[], &["main"]), none);
        assert!(st.world_map.map.current_map().is_none());
        assert_eq!(ok(vm, st, 5206, &[3222 << 14 | 3222], &[]), text(""));
        assert_eq!(ok(vm, st, 5207, &[], &["main"]), text(""));
        assert_eq!(ok(vm, st, 5208, &[], &[]), (vec![0, 0], vec![]));
        // `originX + viewX` and `originY + length - viewY - 1` of zeros.
        assert_eq!(ok(vm, st, 5209, &[], &[]), (vec![0, -1], vec![]));
        assert_eq!(ok(vm, st, 5210, &[], &[]), (vec![0, 0], vec![]));
        assert_eq!(ok(vm, st, 5211, &[], &[]), (vec![0, 0], vec![]));
        assert_eq!(ok(vm, st, 5212, &[], &[]), (vec![0], vec![String::new()]));
        assert_eq!(ok(vm, st, 5213, &[], &[]), (vec![0], vec![String::new()]));
        assert_eq!(
            ok(vm, st, 5215, &[5, 3222 << 14 | 3222], &["main"]),
            (vec![5, 0], vec![])
        );
        assert_eq!(ok(vm, st, 5216, &[3], &[]), none);
        assert_eq!(ok(vm, st, 5217, &[3], &[]), (vec![0], vec![]));
        assert_eq!(ok(vm, st, 5218, &[], &[]), (vec![-1], vec![]));
        assert_eq!(ok(vm, st, 5220, &[], &[]), (vec![0], vec![]));
        assert!(st.diagnostics.is_empty());
    }

    /// Transcription test: `panToCoords` assigns `panTargetX` and then reads
    /// `component.width`, so without the component the Java throws there.
    #[test]
    fn panning_needs_the_component_as_in_the_original() {
        let (mut state, mut vm) = setup();
        let packed = 3000 << 14 | 3100;
        assert_eq!(
            run(&mut vm, &mut state, 5214, &[packed], &[]).unwrap_err(),
            NO_COMPONENT
        );
        assert_eq!(state.world_map.map.pan_target_x, 3000);
        assert_eq!(state.world_map.map.pan_target_y, -1);
        // With the component, and `zoom` still 0.0F, `(int) (width / zoom)`
        // is Integer.MAX_VALUE. `viewLeft` is negative, so `panTargetX`
        // becomes that; `viewRight` and `viewTop` wrap past their tests; and
        // `length < viewBottom` leaves `panTargetY = 0 - MAX_VALUE`.
        state.world_map.map.component = Some([400, 300]);
        run(&mut vm, &mut state, 5214, &[packed], &[]).unwrap();
        let map = &state.world_map.map;
        assert_eq!([map.pan_target_x, map.pan_target_y], [i32::MAX, -i32::MAX]);
    }

    #[test]
    fn stack_underflow_and_unknown_opcodes_are_errors() {
        let (mut state, mut vm) = setup();
        for op in [5200, 5202, 5206, 5214, 5216, 5217] {
            assert!(run(&mut vm, &mut state, op, &[], &[]).is_err(), "{op}");
        }
        for op in [5203, 5204, 5205, 5207, 5219] {
            assert!(run(&mut vm, &mut state, op, &[], &[]).is_err(), "{op}");
        }
        // 5215 pops its integer, then its string.
        assert!(run(&mut vm, &mut state, 5215, &[1], &[]).is_err());
        assert!(run(&mut vm, &mut state, 5215, &[], &["main"]).is_err());
        assert!(run(&mut vm, &mut state, 5221, &[], &[]).is_err());
    }

    /// The queued effects become packets: `::tele 0,x>>6,y>>6,x&63,y&63` as
    /// CLIENT_CHEAT (44) followed by CLOSE_MODAL (184) for a staff click, and
    /// NO_TIMEOUT (93) per forced ping.
    #[test]
    fn queued_teleports_and_pings_become_packets() {
        let (mut state, mut vm) = setup();
        state.world_map.teleports.push([3222, 3218]);
        state.world_map.pings = 2;
        vm.world_map_requests(&mut state);
        assert_eq!(
            vm.outgoing.iter().map(|f| f.opcode).collect::<Vec<_>>(),
            [44, 184, 93, 93]
        );
        // 3222 = 50 * 64 + 22 and 3218 = 50 * 64 + 18; the plane is 0.
        assert_eq!(vm.outgoing[0].payload, b"tele 0,50,50,22,18\0");
        // The same bytes a typed `::tele` line produces.
        let typed = crate::ui::chat::public(
            "::tele 0,50,50,22,18",
            &rs09_protocol::huffman::Huffman::new(&[]).unwrap(),
            0,
            false,
            false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            (typed.opcode, &typed.payload),
            (44, &vm.outgoing[0].payload)
        );
        assert!(state.diagnostics.is_empty());
        assert!(state.world_map.teleports.is_empty());
        assert_eq!(state.world_map.pings, 0);
    }

    /// Every command against the `main` map of the pinned cache.
    ///
    /// Transcription test: the expected values are worked out by hand from
    /// the `ScriptRunner` branches and from the map facts `tests/world_map.rs`
    /// holds from the JVM (one map `main` "RuneScape surface", centre square
    /// 50,50, default zoom 75, display bounds x 2048..=3903 and y
    /// 2496..=4159, 98 map functions, 211 labels of which "Lumbridge" is at
    /// 3239,3234). The commands themselves were not run in a JVM.
    #[test]
    fn every_command_against_the_main_map_of_the_pinned_cache() {
        use crate::ui::world_map_host::Startup;
        let cache = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cache");
        if !cache.join("main_file_cache.dat2").exists() {
            eprintln!(
                "skipped: {} is absent, so no command ran against a loaded map",
                cache.display()
            );
            return;
        }
        let (mut state, mut vm) = setup();
        state
            .world_map
            .open_cache(
                &cache,
                Startup {
                    jitters: Some(([3, -4, 7], [-6, 5, 9])),
                    seed: 1,
                    typeface: None,
                },
            )
            .unwrap();
        let (st, vm) = (&mut state, &mut vm);
        let ok = |vm: &mut Vm, st: &mut State, op, ints: &[i32], strings: &[&str]| {
            run(vm, st, op, ints, strings).unwrap_or_else(|e| panic!("opcode {op}: {e}"))
        };
        let ints = |values: &[i32]| (values.to_vec(), Vec::<String>::new());
        let text = |value: &str| (vec![], vec![value.to_owned()]);
        let none = ints(&[]);
        let packed = |x: i32, y: i32| x << 14 | y;

        // The list is there before any map is selected.
        assert_eq!(ok(vm, st, 5206, &[packed(3222, 3222)], &[]), text("main"));
        assert_eq!(ok(vm, st, 5206, &[packed(3222, 9000)], &[]), text(""));
        // Only the low 14 bits of each half are a coordinate.
        assert_eq!(
            ok(vm, st, 5206, &[packed(3222, 3222) | 1 << 28], &[]),
            text("main")
        );
        assert_eq!(ok(vm, st, 5207, &[], &["main"]), text("RuneScape surface"));
        assert_eq!(ok(vm, st, 5207, &[], &["Main"]), text(""));
        // 5215 compares the group ignoring case; 5207 does not.
        assert_eq!(
            ok(vm, st, 5215, &[packed(3222, 3222)], &["MAIN"]),
            ints(&[1])
        );
        assert_eq!(
            ok(vm, st, 5215, &[packed(3222, 3222)], &["mainland"]),
            ints(&[0])
        );
        assert_eq!(
            ok(vm, st, 5215, &[packed(3222, 9000)], &["main"]),
            ints(&[0])
        );
        assert_eq!(ok(vm, st, 5218, &[], &[]), ints(&[-1]));
        assert_eq!(ok(vm, st, 5210, &[], &[]), ints(&[0, 0]));
        assert_eq!(ok(vm, st, 5211, &[], &[]), ints(&[0, 0]));

        // 5205 selects the map; an unknown name clears and selects nothing.
        assert_eq!(ok(vm, st, 5205, &[], &["nowhere"]), none);
        assert_eq!(ok(vm, st, 5218, &[], &[]), ints(&[-1]));
        assert_eq!(ok(vm, st, 5205, &[], &["main"]), none);
        assert_eq!(ok(vm, st, 5218, &[], &[]), ints(&[75]));
        assert_eq!(ok(vm, st, 5210, &[], &[]), ints(&[50 * 64, 50 * 64]));
        assert_eq!(
            ok(vm, st, 5211, &[], &[]),
            ints(&[3903 - 2048, 4159 - 2496])
        );
        assert_eq!(ok(vm, st, 5220, &[], &[]), ints(&[0]));
        assert_eq!(ok(vm, st, 5212, &[], &[]), (vec![0], vec![String::new()]));

        // The component draws: one load step per frame until it is loaded.
        let (width, height) = (512, 334);
        let mut frames = 0;
        while !st.world_map.map.is_loaded() {
            frames += 1;
            assert!(frames <= 20, "the map did not load");
            let rendered = st
                .world_map
                .render_map([3222, 3218], &|_, _| 0, width, height)
                .unwrap();
            assert_eq!(
                rendered.loading_text.is_some(),
                !st.world_map.map.is_loaded()
            );
        }
        // The JVM's load of this map: 14 steps, 10 forced pings, one
        // `client.clearScene()`, 13 timer resets.
        assert_eq!(frames, 14);
        assert_eq!(st.world_map.pings, 10);
        let scene = st.world_map.scene_requests();
        assert_eq!(
            [scene.clear_scene, scene.reset_timer, scene.rebuild_scene],
            [1, 13, 0]
        );
        assert_eq!(ok(vm, st, 5220, &[], &[]), ints(&[1]));
        // The view starts within five tiles of the player.
        let (view, _) = ok(vm, st, 5209, &[], &[]);
        assert!(
            (view[0] - 3222).abs() <= 5 && (view[1] - 3218).abs() <= 5,
            "{view:?}"
        );

        // 5200/5201: only the five stops are accepted.
        assert_eq!(ok(vm, st, 5201, &[], &[]), ints(&[75]));
        for (level, zoom) in [(37, 3.0), (50, 4.0), (100, 8.0), (200, 16.0), (75, 6.0)] {
            assert_eq!(ok(vm, st, 5200, &[level], &[]), none);
            assert_eq!(st.world_map.map.target_zoom, zoom);
            assert_eq!(ok(vm, st, 5201, &[], &[]), ints(&[level]));
        }
        assert_eq!(ok(vm, st, 5200, &[60], &[]), none);
        assert_eq!(ok(vm, st, 5201, &[], &[]), ints(&[75]));

        // 5208/5209 after a drawn frame at zoom 6 with a known view.
        st.world_map.map.view_x = 3222 - 2048;
        st.world_map.map.view_y = 2496 + 1664 - 3218 - 1;
        st.world_map
            .render_map([3222, 3218], &|_, _| 0, width, height)
            .unwrap();
        assert_eq!(
            ok(vm, st, 5208, &[], &[]),
            ints(&[width * 2 / 6, height * 2 / 6])
        );
        assert_eq!(ok(vm, st, 5209, &[], &[]), ints(&[3222, 3218]));

        // 5212/5213 list the visible labels in order, with `<br>` as a space,
        // and then keep answering the empty string and 0.
        let labels = st.world_map.map.labels.clone().unwrap();
        assert_eq!(labels.count(), 211);
        let mut listed = Vec::new();
        let mut op = 5212;
        loop {
            let (sizes, names) = ok(vm, st, op, &[], &[]);
            op = 5213;
            if names[0].is_empty() {
                assert_eq!(sizes, [0]);
                break;
            }
            listed.push((names[0].clone(), sizes[0]));
            assert!(listed.len() <= 211);
        }
        assert_eq!(ok(vm, st, 5213, &[], &[]), (vec![0], vec![String::new()]));
        let expected: Vec<(String, i32)> = (0..211)
            .filter(|&i| i32::from(labels.flags[i]) & 16 == 0)
            .map(|i| {
                let name: String = labels.names[i].iter().map(|&b| b as char).collect();
                (name.replace("<br>", " "), i32::from(labels.flags[i]) & 3)
            })
            .collect();
        assert_eq!(listed, expected);
        assert!(!listed.is_empty());
        let broken = (0..211)
            .find(|&i| labels.names[i].windows(4).any(|w| w == b"<br>"))
            .expect("a label of more than one line");
        let raw: String = labels.names[broken].iter().map(|&b| b as char).collect();
        assert!(listed.iter().all(|(name, _)| !name.contains("<br>")));
        // 5212 restarts the listing.
        assert_eq!(
            ok(vm, st, 5212, &[], &[]),
            (vec![listed[0].1], vec![listed[0].0.clone()])
        );

        // 5204: the first label with the prefix, ignoring case; the top of
        // the string stack is replaced.
        let (_, found) = ok(vm, st, 5204, &[], &["below", "LUMBRIDGE"]);
        assert_eq!(found[0], "below");
        assert!(found[1].starts_with("Lumbridge"), "{found:?}");
        assert_eq!(ok(vm, st, 5204, &[], &["no such place"]), text(""));
        assert_eq!(ok(vm, st, 5204, &[], &[""]), text(""));
        // A multi-line label is found by its text with spaces.
        let line = raw.replace("<br>", " ");
        let (_, found) = ok(vm, st, 5204, &[], &[&line]);
        assert!(
            found[0].to_lowercase().starts_with(&line.to_lowercase()),
            "{found:?} for {line:?}"
        );

        // 5219 pans to "Lumbridge" at 3239,3234; the component of the frame
        // above is large enough for the target not to be pulled in.
        st.world_map.map.component = Some([width, height]);
        assert_eq!(ok(vm, st, 5219, &[], &["Lumbridge"]), none);
        let target = |st: &State| [st.world_map.map.pan_target_x, st.world_map.map.pan_target_y];
        assert_eq!(target(st), [3239 - 2048, 1664 + 2496 - 3234 - 1]);
        // The exact name is case-sensitive: nothing is found, nothing moves.
        st.world_map.map.pan_target_x = -1;
        st.world_map.map.pan_target_y = -1;
        assert_eq!(ok(vm, st, 5219, &[], &["lumbridge"]), none);
        assert_eq!(target(st), [-1, -1]);
        // 5203 pans to the label 5204 names.
        let index = st.world_map.map.find_label_by_prefix("lumbridge");
        assert!(index >= 0);
        assert_eq!(ok(vm, st, 5203, &[], &["lumbridge"]), none);
        assert_eq!(
            target(st),
            [
                i32::from(labels.coord_x[index as usize]) - 2048,
                1664 + 2496 - i32::from(labels.coord_y[index as usize]) - 1
            ]
        );
        // 5214 pans to a packed coordinate; near the left edge the target is
        // held half a view (512 / 6 tiles) inside the map.
        assert_eq!(ok(vm, st, 5214, &[packed(3000, 3100)], &[]), none);
        assert_eq!(target(st), [3000 - 2048, 1664 + 2496 - 3100 - 1]);
        assert_eq!(ok(vm, st, 5214, &[packed(2050, 3100)], &[]), none);
        assert_eq!(target(st), [512 / 6, 1664 + 2496 - 3100 - 1]);
        // Without the component the original throws, for a found label too.
        st.world_map.map.component = None;
        assert_eq!(
            run(vm, st, 5214, &[packed(3000, 3100)], &[]).unwrap_err(),
            NO_COMPONENT
        );
        assert_eq!(
            run(vm, st, 5219, &[], &["Lumbridge"]).unwrap_err(),
            NO_COMPONENT
        );
        assert_eq!(ok(vm, st, 5219, &[], &["no such place"]), none);

        // 5202 starts three pulses on a map function.
        assert_eq!(ok(vm, st, 5202, &[12], &[]), none);
        let map = &st.world_map.map;
        assert_eq!(
            [
                map.highlighted_map_function,
                map.highlight_pulse_count,
                map.highlight_pulse_counter
            ],
            [12, 3, 20]
        );

        // 5216/5217: one toggle per map function; indices outside the 98
        // sprites change and report nothing, in the Java as here.
        assert_eq!(ok(vm, st, 5217, &[5], &[]), ints(&[0]));
        assert_eq!(ok(vm, st, 5216, &[5], &[]), none);
        assert_eq!(ok(vm, st, 5217, &[5], &[]), ints(&[1]));
        assert_eq!(ok(vm, st, 5217, &[6], &[]), ints(&[0]));
        assert_eq!(ok(vm, st, 5216, &[5], &[]), none);
        assert_eq!(ok(vm, st, 5217, &[5], &[]), ints(&[0]));
        for index in [-1, 98, i32::MAX] {
            assert_eq!(ok(vm, st, 5216, &[index], &[]), none);
            assert_eq!(ok(vm, st, 5217, &[index], &[]), ints(&[0]));
        }
        assert!(st.diagnostics.is_empty());
    }
}
