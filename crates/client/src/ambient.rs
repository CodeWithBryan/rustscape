//! Original single-location AreaSound construction, morph update and redraw.
//! Collection/actor services and PCM readiness/linkage are explicit bindings.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Definition {
    pub sound: i32,
    /// Fine-coordinate radius, after LocType's byte range is multiplied by128.
    pub radius: i32,
    pub intervals: [i32; 2],
    pub sounds: Option<Vec<i32>>,
}
impl Default for Definition {
    fn default() -> Self {
        Self {
            sound: -1,
            radius: 0,
            intervals: [0; 2],
            sounds: None,
        }
    }
}
impl Definition {
    pub fn audible(&self) -> bool {
        self.sound != -1 || self.sounds.is_some()
    }
}

pub trait Host {
    fn start(&mut self, id: i32, volume: i32, loops: i32) -> Option<u64>;
    fn volume(&mut self, id: u64, volume: i32);
    fn stop(&mut self, id: u64);
    fn linked(&self, id: u64) -> bool;
}
#[derive(Clone, Debug, Serialize)]
pub struct Emitter {
    pub min: [i32; 2],
    pub max: [i32; 2],
    pub plane: i32,
    pub definition: Definition,
    pub remaining: i32,
    pub multi: bool,
    pub primary: Option<u64>,
    pub secondary: Option<u64>,
}
impl Emitter {
    pub fn location(
        tile: [i32; 2],
        mut size: [i32; 2],
        rotation: i32,
        plane: i32,
        definition: Definition,
        morph: Option<Option<Definition>>,
        mut random: impl FnMut() -> f64,
    ) -> Self {
        if rotation == 1 || rotation == 3 {
            size.swap(0, 1);
        }
        let multi = morph.is_some();
        let definition = morph.map_or(definition, |d| d.unwrap_or_default());
        let remaining = if definition.sounds.is_some() {
            definition.intervals[0].wrapping_add(
                (random()
                    * f64::from(definition.intervals[1].wrapping_sub(definition.intervals[0])))
                    as i32,
            )
        } else {
            0
        };
        Self {
            min: tile.map(|n| n.wrapping_mul(128)),
            max: std::array::from_fn(|i| tile[i].wrapping_add(size[i]).wrapping_mul(128)),
            plane,
            definition,
            remaining,
            multi,
            primary: None,
            secondary: None,
        }
    }
    pub fn update(&mut self, definition: Option<Definition>, host: &mut impl Host) {
        let old = self.definition.sound;
        self.definition = definition.unwrap_or_default();
        if old != self.definition.sound {
            if let Some(id) = self.primary.take() {
                host.stop(id);
            }
        }
    }
    pub fn remove(&mut self, host: &mut impl Host) {
        if let Some(id) = self.primary.take() {
            host.stop(id);
        }
        if let Some(id) = self.secondary.take() {
            host.stop(id);
        }
    }
    pub fn tick(
        &mut self,
        listener: [i32; 2],
        plane: i32,
        elapsed: i32,
        volume: i32,
        host: &mut impl Host,
        mut random: impl FnMut() -> f64,
    ) {
        let d = &self.definition;
        if !d.audible() {
            return;
        }
        let mut distance = 0i32;
        for (i, &fine) in listener.iter().enumerate() {
            if self.max[i] < fine {
                distance = distance.wrapping_add(fine.wrapping_sub(self.max[i]));
            } else if self.min[i] > fine {
                distance = distance.wrapping_add(self.min[i].wrapping_sub(fine));
            }
        }
        if d.radius == 0
            || d.radius < distance.wrapping_sub(64)
            || volume == 0
            || plane != self.plane
        {
            self.remove(host);
            return;
        }
        distance = distance.wrapping_sub(64).max(0);
        let volume = d
            .radius
            .wrapping_sub(distance)
            .wrapping_mul(volume)
            .wrapping_div(d.radius);
        if let Some(id) = self.primary {
            host.volume(id, volume);
        } else if d.sound >= 0 {
            self.primary = host.start(d.sound, volume, -1);
        }
        if let Some(id) = self.secondary {
            host.volume(id, volume);
            if !host.linked(id) {
                self.secondary = None;
            }
        } else if let Some(sounds) = &d.sounds {
            self.remaining = self.remaining.wrapping_sub(elapsed);
            if self.remaining <= 0 {
                // Empty/out-of-range lists are bounded malformed metadata,
                // rather than reproducing the original array exception.
                if let Some(&sound) = sounds.get((sounds.len() as f64 * random()) as usize) {
                    if let Some(id) = host.start(sound, volume, 0) {
                        self.remaining = (f64::from(d.intervals[1].wrapping_sub(d.intervals[0]))
                            * random()) as i32;
                        self.remaining = self.remaining.wrapping_add(d.intervals[0]);
                        self.secondary = Some(id);
                    }
                }
            }
        }
    }
}
