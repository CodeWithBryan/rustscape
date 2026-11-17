//! Original SoundPlayer finite-effect queue, sequence selection and area attenuation.
//! Cache readiness/start trimming, random choices and PCM playback are bindings.
use crate::animation::Sequence;
/// Original SYNTH_SOUND field reads. The second byte-sized field is a loop
/// count despite the original reader's local variable being named volume.
pub fn synth_frame(frame: &rs09_protocol::game::Frame) -> rs09_protocol::Result<Option<[i32; 3]>> {
    if frame.opcode != 172 {
        return Ok(None);
    }
    let mut r = rs09_protocol::buffer::Reader::new(&frame.payload);
    let id = r.g2()?;
    let loops = r.g1()?;
    let delay = r.g2()?;
    if r.remaining() != 0 {
        return Err(rs09_protocol::Error::new(
            r.position(),
            "trailing synthesized sound bytes",
        ));
    }
    Ok(Some([if id == 65535 { -1 } else { id }, loops, delay]))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: i32,
    pub loops: i32,
    pub delay: i32,
    pub position: i32,
    pub start: Option<i32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playback {
    pub id: i32,
    pub loops: i32,
    pub volume: i32,
}
#[derive(Default)]
pub struct Queue {
    pub entries: Vec<Entry>,
}
impl Queue {
    pub fn play(&mut self, loops: i32, id: i32, delay: i32, volume: i32) {
        if volume == 0 || loops == 0 || self.entries.len() >= 50 || id == -1 {
            return;
        }
        self.entries.push(Entry {
            id,
            loops,
            delay,
            position: 0,
            start: None,
        });
    }
    pub fn sequence(
        &mut self,
        seq: &Sequence,
        frame: i32,
        fine: [i32; 2],
        is_self: bool,
        volumes: [i32; 2],
        mut random: impl FnMut() -> f64,
    ) {
        if self.entries.len() >= 50 {
            return;
        }
        let Some(sounds) = usize::try_from(frame)
            .ok()
            .and_then(|n| seq.sounds.get(n))
            .filter(|s| !s.is_empty())
        else {
            return;
        };
        let packed = sounds[0] as i32;
        let mut id = packed >> 8;
        if sounds.len() > 1 {
            let index = (sounds.len() as f64 * random()) as usize;
            if index > 0 {
                let Some(&alternative) = sounds.get(index) else {
                    return;
                };
                id = alternative as i32;
            }
        }
        let loops = (packed >> 5) & 7;
        let distance = packed & 31;
        if distance == 0 {
            if is_self {
                self.play(loops, id, 0, volumes[0]);
            }
        } else if volumes[1] != 0 {
            let x = fine[0].wrapping_sub(64) / 128;
            let y = fine[1].wrapping_sub(64) / 128;
            self.entries.push(Entry {
                id,
                loops,
                delay: 0,
                position: distance
                    .wrapping_add(x.wrapping_shl(16))
                    .wrapping_add(y.wrapping_shl(8)),
                start: None,
            });
        }
    }
    /// SOUND_AREA admission, evaluated when the packet is read: the listener's
    /// queue-head tile must lie within `radius + 1` tiles on both axes. Tiles
    /// are scene-local; the packed position keeps the radius in its low byte.
    pub fn area(
        &mut self,
        tile: [i32; 2],
        listener: [i32; 2],
        [id, radius, loops, delay]: [i32; 4],
        ambient_volume: i32,
    ) {
        if tile.into_iter().any(|n| !(0..104).contains(&n)) {
            return;
        }
        let range = radius + 1;
        let near = (0..2).all(|i| listener[i] >= tile[i] - range && listener[i] <= tile[i] + range);
        if near && ambient_volume != 0 && loops > 0 && self.entries.len() < 50 && id != -1 {
            self.entries.push(Entry {
                id,
                loops,
                delay,
                position: radius + (tile[0] << 16) + (tile[1] << 8),
                start: None,
            });
        }
    }
    /// Exactly one original logical client cycle. Missing cache data retries
    /// until the original -10 delay cutoff; expired entries retain their slot
    /// until the following call, preserving the shared fifty-entry bound.
    pub fn tick(
        &mut self,
        listener: [i32; 2],
        volumes: [i32; 2],
        mut load: impl FnMut(i32) -> Option<i32>,
    ) -> Vec<Playback> {
        let mut output = vec![];
        let mut i = 0;
        while i < self.entries.len() {
            let e = &mut self.entries[i];
            e.delay = e.delay.wrapping_sub(1);
            if e.delay < -10 {
                self.entries.remove(i);
                continue;
            }
            if e.start.is_none() {
                let Some(start) = load(e.id) else {
                    i += 1;
                    continue;
                };
                e.delay = e.delay.wrapping_add(start);
                e.start = Some(start);
            }
            if e.delay < 0 {
                let volume = if e.position == 0 {
                    volumes[0]
                } else {
                    let radius = (e.position & 255) * 128;
                    let y = ((e.position >> 8) & 255) * 128 + 64;
                    let x = ((e.position >> 16) & 255) * 128 + 64;
                    let mut dy = y.wrapping_sub(listener[1]);
                    if dy < 0 {
                        dy = dy.wrapping_neg();
                    }
                    let mut dx = x.wrapping_sub(listener[0]);
                    if dx < 0 {
                        dx = dx.wrapping_neg();
                    }
                    let distance = dx.wrapping_add(dy).wrapping_sub(128);
                    if radius < distance {
                        e.delay = -100;
                        i += 1;
                        continue;
                    }
                    if radius == 0 {
                        e.delay = -100;
                        i += 1;
                        continue;
                    }
                    volumes[1].wrapping_mul(radius.wrapping_sub(distance.max(0))) / radius
                };
                if volume > 0 {
                    output.push(Playback {
                        id: e.id,
                        loops: e.loops,
                        volume,
                    });
                }
                e.delay = -100;
            }
            i += 1;
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_sounds_are_admitted_by_listener_range_and_attenuate_by_distance() {
        let mut q = Queue::default();
        // Radius 2 admits a listener up to three tiles away on each axis.
        q.area([10, 20], [13, 17], [50, 2, 1, 0], 127);
        assert_eq!(
            q.entries,
            [Entry {
                id: 50,
                loops: 1,
                delay: 0,
                position: 2 + (10 << 16) + (20 << 8),
                start: None
            }]
        );
        for (tile, listener, fields, volume) in [
            ([10, 20], [14, 20], [50, 2, 1, 0], 127),
            ([10, 20], [10, 16], [50, 2, 1, 0], 127),
            ([104, 20], [104, 20], [50, 2, 1, 0], 127),
            ([-1, 20], [0, 20], [50, 2, 1, 0], 127),
            ([10, 20], [10, 20], [50, 2, 0, 0], 127),
            ([10, 20], [10, 20], [-1, 2, 1, 0], 127),
            ([10, 20], [10, 20], [50, 2, 1, 0], 0),
        ] {
            q.area(tile, listener, fields, volume);
            assert_eq!(
                q.entries.len(),
                1,
                "{tile:?} {listener:?} {fields:?} {volume}"
            );
        }
        // Standing on the source tile plays at the full ambient volume; the
        // Manhattan fine distance (less one tile) scales it down to silence.
        let centre = [10 * 128 + 64, 20 * 128 + 64];
        let played = q.tick(centre, [127, 100], |_| Some(0));
        assert_eq!(
            played,
            [Playback {
                id: 50,
                loops: 1,
                volume: 100
            }]
        );
        let mut q = Queue::default();
        q.area([10, 20], [12, 20], [50, 2, 1, 0], 127);
        let played = q.tick([centre[0] + 256, centre[1]], [127, 100], |_| Some(0));
        assert_eq!(played[0].volume, 100 * (256 - 128) / 256);
        let mut q = Queue::default();
        q.area([10, 20], [13, 20], [50, 2, 1, 0], 127);
        assert!(
            q.tick([centre[0] + 400, centre[1]], [127, 100], |_| Some(0))
                .is_empty()
        );
    }

    #[test]
    fn the_queue_holds_at_most_fifty_entries() {
        let mut q = Queue::default();
        for _ in 0..60 {
            q.area([1, 1], [1, 1], [5, 0, 1, 0], 127);
        }
        assert_eq!(q.entries.len(), 50);
    }
}
