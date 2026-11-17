//! Components the client itself drives by `clientCode`:
//! `Cs1ScriptRunner.applyClientCode` (1202-1243) for 324, 325, 327 and 328,
//! and the debug overlay component 1405 (`renderComponent` 486-533).
//!
//! In the pinned cache only 328 (the body on the character design screens,
//! for example 771:339) and 1405 (548:3, 744:117, 746:1) occur; 324, 325 and
//! 327 are ported from the code alone.
use super::Component;

/// What the original reads from the rest of the client.
#[derive(Clone, Copy, Debug, Default)]
pub struct Inputs {
    /// `client.loop`.
    pub cycle: i32,
    /// `PlayerAppearance.DEFAULT.gender`. This client only ever resets the
    /// default appearance to male (`client.java` 722), so the original's
    /// value is always false.
    pub default_female: bool,
    /// `PlayerList.self.username.encode37()`, or None while the local player
    /// has no name.
    pub self_name37: Option<i64>,
    /// The local player's `movementSeqId`, `movementSeqFrame` and
    /// `movementSeqNextFrame`.
    pub movement: [i32; 3],
}

/// `cachedDefaultSpriteId` and `cachedActiveSpriteId`: one pair shared by
/// 324 and 325, taken from whichever such component is drawn first.
#[derive(Clone, Copy, Debug)]
pub struct GenderSprites {
    pub default: i32,
    pub active: i32,
}
impl Default for GenderSprites {
    fn default() -> Self {
        Self {
            default: -1,
            active: 0,
        }
    }
}

/// `(int) (Math.sin((double) loop / 40.0D) * 256.0D) & 0x7FF`.
pub fn sway(cycle: i32) -> i32 {
    ((f64::from(cycle) / 40.0).sin() * 256.0) as i32 & 0x7ff
}

/// `applyClientCode`, called for every drawn component with a client code.
pub fn apply(c: &mut Component, sprites: &mut GenderSprites, inputs: &Inputs) {
    match c.client_code {
        324 | 325 => {
            if sprites.default == -1 {
                sprites.default = c.sprite_id;
                sprites.active = c.active_sprite_id;
            }
            // 324 shows the default sprite for a female default appearance,
            // 325 the other way round.
            c.sprite_id = if inputs.default_female == (c.client_code == 324) {
                sprites.default
            } else {
                sprites.active
            };
        }
        327 => {
            c.model_x_angle = 150;
            c.model_y_angle = sway(inputs.cycle);
            c.model_type = 5;
            c.model_id = -1;
        }
        328 => match inputs.self_name37 {
            None => c.model_id = 0,
            Some(name) => {
                c.model_x_angle = 150;
                c.model_y_angle = sway(inputs.cycle);
                c.model_type = 5;
                c.model_id = (name as i32).wrapping_shl(11).wrapping_add(2047);
                c.seq_next_frame = inputs.movement[2];
                c.seq_cycle = 0;
                c.model_seq_id = inputs.movement[0];
                c.seq_frame = inputs.movement[1];
            }
        },
        _ => {}
    }
}

/// The figures of the 1405 overlay. The original reads the JVM heap, the GL
/// allocation counters and the JS5 verification counts; a figure with no
/// counterpart here is None and its line is left out.
#[derive(Clone, Copy, Debug, Default)]
pub struct Overlay {
    /// `(int) GameShell.framesPerSecond`.
    pub fps: i32,
    /// Megabytes of heap in use.
    pub memory: Option<i32>,
    /// Megabytes on the graphics card.
    pub card: Option<i32>,
    /// `[verified * 10000 / size, total verified * 100 / size]` over the
    /// 28 archives.
    pub cache: Option<[i32; 2]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OverlayLine {
    pub text: String,
    /// The right edge and baseline.
    pub at: [i32; 2],
    pub color: i32,
    /// False for p12, true for the p11 cache line.
    pub small: bool,
}

/// The lines `renderComponent` draws for client code 1405 while
/// `Cheat.displayFps` is set, each right-aligned with a black shadow. Every
/// line keeps the position the original gives it, whether or not the lines
/// above it are present.
pub fn overlay(origin: [i32; 2], width: i32, values: Overlay) -> Vec<OverlayLine> {
    const YELLOW: i32 = 16776960;
    const RED: i32 = 16711680;
    let x = width.wrapping_add(origin[0]);
    let mut y = origin[1].wrapping_add(15);
    let mut lines = vec![OverlayLine {
        text: format!("FPS: {}", values.fps),
        at: [x, y],
        color: YELLOW,
        small: false,
    }];
    y = y.wrapping_add(15);
    if let Some(memory) = values.memory {
        lines.push(OverlayLine {
            text: format!("RAM: {memory} MB"),
            at: [x, y],
            color: if memory > 128 { RED } else { YELLOW },
            small: false,
        });
    }
    y = y.wrapping_add(15);
    // The original draws this line in HD only; this client is always HD.
    if let Some(card) = values.card {
        lines.push(OverlayLine {
            text: format!("GPU: {card} MB"),
            at: [x, y],
            color: if card > 64 { RED } else { YELLOW },
            small: false,
        });
    }
    y = y.wrapping_add(15);
    if let Some([verified, total]) = values.cache {
        lines.push(OverlayLine {
            text: format!("Cache: {}% ({}%)", two_decimals(i64::from(verified)), total),
            at: [x, y],
            color: YELLOW,
            small: true,
        });
    }
    lines
}

/// `StringUtils.formatNumber(0, true, 2, value)`: two decimal places and
/// comma grouping of the integer part.
fn two_decimals(value: i64) -> String {
    let (sign, value) = if value < 0 {
        ("-", value.wrapping_neg())
    } else {
        ("", value)
    };
    // The original builds the number backwards, least significant digit
    // first, and reverses it at the end.
    let fraction = format!("{}{}", value / 10 % 10, value % 10);
    let mut whole = value / 100;
    let mut digits = String::new();
    let mut count = 0;
    while whole > 0 {
        if count != 0 && count % 3 == 0 {
            digits.insert(0, ',');
        }
        digits.insert(0, char::from(b'0' + (whole % 10) as u8));
        count += 1;
        whole /= 10;
    }
    if digits.is_empty() {
        digits.push('0');
    }
    format!("{sign}{digits}.{fraction}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests: expected values worked out by hand from
    // Cs1ScriptRunner.applyClientCode and renderComponent 486-533.
    fn component(code: i32) -> Component {
        Component {
            client_code: code,
            sprite_id: 10,
            active_sprite_id: 20,
            model_type: 1,
            model_id: 99,
            ..Component::default()
        }
    }

    #[test]
    fn gender_sprites_share_one_cached_pair() {
        let mut sprites = GenderSprites::default();
        let male = Inputs::default();
        let female = Inputs {
            default_female: true,
            ..male
        };
        let mut a = component(324);
        apply(&mut a, &mut sprites, &male);
        assert_eq!(a.sprite_id, 20);
        // The pair is not taken again: a second component with other
        // sprites is given the first one's.
        let mut b = component(325);
        b.sprite_id = 30;
        b.active_sprite_id = 40;
        apply(&mut b, &mut sprites, &male);
        assert_eq!(b.sprite_id, 10);
        apply(&mut a, &mut sprites, &female);
        apply(&mut b, &mut sprites, &female);
        assert_eq!((a.sprite_id, b.sprite_id), (10, 20));
        assert_eq!((a.active_sprite_id, b.active_sprite_id), (20, 40));
    }

    #[test]
    fn bodies_turn_with_the_client_cycle() {
        assert_eq!(sway(0), 0);
        // sin(20 / 40) * 256 = 122.7; sin(100 / 40) * 256 = 153.2.
        assert_eq!(sway(20), 122);
        assert_eq!(sway(100), 153);
        // sin(160 / 40) * 256 = -193.7, truncated to -193, masked to 11 bits.
        assert_eq!(sway(160), (-193) & 0x7ff);
        let mut sprites = GenderSprites::default();
        let mut c = component(327);
        apply(
            &mut c,
            &mut sprites,
            &Inputs {
                cycle: 20,
                ..Inputs::default()
            },
        );
        assert_eq!(
            (c.model_x_angle, c.model_y_angle, c.model_type, c.model_id),
            (150, 122, 5, -1)
        );
    }

    #[test]
    fn the_local_body_follows_the_players_movement_animation() {
        let mut sprites = GenderSprites::default();
        let mut c = component(328);
        c.seq_cycle = 7;
        apply(&mut c, &mut sprites, &Inputs::default());
        // No name yet: only the model id changes.
        assert_eq!((c.model_id, c.model_type, c.seq_cycle), (0, 1, 7));
        let inputs = Inputs {
            cycle: 100,
            self_name37: Some(0x1_2345_6789),
            movement: [808, 3, 4],
            ..Inputs::default()
        };
        apply(&mut c, &mut sprites, &inputs);
        assert_eq!(
            c.model_id,
            0x2345_6789i32.wrapping_shl(11).wrapping_add(2047)
        );
        assert_eq!(c.model_id & 2047, 2047);
        assert_eq!(
            (c.model_x_angle, c.model_y_angle, c.model_type),
            (150, 153, 5)
        );
        assert_eq!(
            (c.model_seq_id, c.seq_frame, c.seq_next_frame, c.seq_cycle),
            (808, 3, 4, 0)
        );
    }

    #[test]
    fn overlay_lines_keep_their_rows() {
        let lines = overlay(
            [600, 4],
            150,
            Overlay {
                fps: 50,
                memory: Some(200),
                card: Some(64),
                cache: Some([9876, 98]),
            },
        );
        assert_eq!(
            lines
                .iter()
                .map(|l| (l.text.as_str(), l.at, l.color, l.small))
                .collect::<Vec<_>>(),
            vec![
                ("FPS: 50", [750, 19], 16776960, false),
                ("RAM: 200 MB", [750, 34], 16711680, false),
                ("GPU: 64 MB", [750, 49], 16776960, false),
                ("Cache: 98.76% (98%)", [750, 64], 16776960, true),
            ]
        );
        let only = overlay(
            [0, 0],
            100,
            Overlay {
                fps: 7,
                ..Overlay::default()
            },
        );
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].at, [100, 15]);
        assert_eq!(two_decimals(5), "0.05");
        assert_eq!(two_decimals(123456789), "1,234,567.89");
    }
}
