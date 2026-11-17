//! The logged-out client outside the interface system: the cache names the
//! title screen is found by, the loading bar of `client.mainLoad`
//! (`LoadingBar.render`), and the world-list stage that ends the load.
//!
//! The title screen itself is an interface (`loginscreen`, group 744 in the
//! pinned cache) whose scripts run the whole screen; see
//! [`crate::login_flow`] for the commands behind them and
//! [`crate::login_seam`] for the game state around them.
use crate::{login_flow::WorldListStartup, pack::Pack};

/// The four groups `client.mainLoad` finds by name: `client.LOGINSCREEN` in
/// archive 3 (state 140), `client.TITLE_SONG` in archive 6 (state 50), and
/// `TitleScreen.TITLEBG` and `TitleScreen.LOGO` in archive 8 (state 60).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Assets {
    /// `LoginManager.loginScreenId`.
    pub interface: i32,
    /// `MusicPlayer.titleSong`.
    pub song: i32,
    /// `TitleScreen.bgId`.
    pub background: i32,
    /// `TitleScreen.logoId`.
    pub logo: i32,
}
impl Assets {
    /// The pack entry that carries the four ids as JSON, keyed by the cache
    /// names. `cache_pack::title_assets` writes it.
    pub const ENTRY: &'static str = "title/assets";
    /// The ids in the pinned cache, for packs prepared before
    /// [`Self::ENTRY`] existed (they hold the same cache).
    pub const PINNED: Self = Self {
        interface: 744,
        song: 0,
        background: 1407,
        logo: 498,
    };
    /// The pack's ids, or the pinned ones when it has no entry. A malformed
    /// entry is an error rather than a silent fallback.
    pub fn read(pack: &Pack) -> Result<Self, String> {
        let Ok(bytes) = pack.get(Self::ENTRY) else {
            return Ok(Self::PINNED);
        };
        let ids: std::collections::BTreeMap<String, i32> =
            serde_json::from_slice(bytes).map_err(|e| format!("{}: {e}", Self::ENTRY))?;
        let id = |name: &str| {
            ids.get(name)
                .copied()
                .ok_or_else(|| format!("{}: no {name}", Self::ENTRY))
        };
        Ok(Self {
            interface: id(crate::login_flow::TITLE_INTERFACE)?,
            song: id(crate::login_flow::TITLE_SONG)?,
            background: id(crate::login_flow::TITLE_BACKGROUND)?,
            logo: id(crate::login_flow::TITLE_LOGO)?,
        })
    }
}

/// `LocalizedText.GAME0_LOADING` (English).
pub const LOADING_TITLE: &str = "RuneScape is loading - please wait...";
/// `LocalizedText.MAINLOAD135` and `MAINLOAD135B` (English).
pub const WORLD_LIST_LOADING: &str = "Loading world list data";
pub const WORLD_LIST_LOADED: &str = "Loaded world list data";

/// `LoadingBar.render` (LoadingBar.java:10-33) for a canvas, a percentage
/// and the logo's width (`IndexedSprite.width`, its uncropped width).
/// Rectangles are `[x, y, width, height]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadingBar {
    /// `titleBg.renderResized`: the 956 by 503 background scaled to the
    /// canvas height and centred.
    pub background: [i32; 4],
    /// `logo.renderTransparent`: where the logo's uncropped origin goes.
    pub logo: [i32; 2],
    /// The centre and baseline of [`LOADING_TITLE`] in `b12_full`.
    pub title: [i32; 2],
    /// `drawRect` in [`Self::RED`]: the outer one-pixel outline.
    pub outline: [i32; 4],
    /// `drawRect` in black: the inner one-pixel outline.
    pub inner_outline: [i32; 4],
    /// `fillRect` in [`Self::RED`]: three pixels per percent.
    pub filled: [i32; 4],
    /// `fillRect` in black: the rest of the 300 pixels.
    pub empty: [i32; 4],
    /// The centre and baseline of the stage text (`mainLoadSecondaryText`).
    pub text: [i32; 2],
}
impl LoadingBar {
    /// 9179409: the bar's colour.
    pub const RED: u32 = 0x8c_1111;
    /// Both texts are drawn in white without a shadow (`-1`).
    pub const TEXT: u32 = 0xff_ffff;
    pub fn layout([width, height]: [i32; 2], percent: i32, logo_width: i32) -> Self {
        let background_width = height * 956 / 503;
        let y = height / 2 - 18;
        Self {
            background: [(width - background_width) / 2, 0, background_width, height],
            logo: [width / 2 - logo_width / 2, 18],
            title: [width / 2, height / 2 - 26],
            outline: [width / 2 - 152, y, 304, 34],
            inner_outline: [width / 2 - 151, y + 1, 302, 32],
            filled: [width / 2 - 150, y + 2, percent * 3, 30],
            empty: [width / 2 + percent * 3 - 150, y + 2, 300 - percent * 3, 30],
            text: [width / 2, height / 2 + 4],
        }
    }
}

/// `client.mainLoad` state 135 (client.java:1644-1659): what the loading bar
/// shows for a world-list fetch result, or the fatal error it ends in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorldListStage {
    /// The bar's percentage and text; `done` moves on to state 140.
    Bar {
        percent: i32,
        text: &'static str,
        done: bool,
    },
    /// `this.error(..)` and game state 1000: the original stops here for
    /// good, showing only the error text.
    Fatal(String),
}
impl WorldListStage {
    pub fn of(result: WorldListStartup) -> Self {
        match result {
            WorldListStartup::Loading => Self::Bar {
                percent: 95,
                text: WORLD_LIST_LOADING,
                done: false,
            },
            WorldListStartup::Loaded => Self::Bar {
                percent: 96,
                text: WORLD_LIST_LOADED,
                done: true,
            },
            WorldListStartup::Full => Self::Fatal("worldlistfull".to_owned()),
            WorldListStartup::Failed(code) => Self::Fatal(format!("worldlistio_{code}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Transcription test: the arithmetic of `LoadingBar.render` worked by
    /// hand for the fixed 765 by 503 canvas at 95% with a 360-wide logo, and
    /// for a wide resizable canvas (the divisions truncate).
    #[test]
    fn the_loading_bar_is_laid_out_as_the_original_draws_it() {
        let bar = LoadingBar::layout([765, 503], 95, 360);
        assert_eq!(bar.background, [-95, 0, 956, 503]);
        assert_eq!(bar.logo, [202, 18]);
        assert_eq!(bar.title, [382, 225]);
        assert_eq!(bar.outline, [230, 233, 304, 34]);
        assert_eq!(bar.inner_outline, [231, 234, 302, 32]);
        assert_eq!(bar.filled, [232, 235, 285, 30]);
        assert_eq!(bar.empty, [517, 235, 15, 30]);
        assert_eq!(bar.text, [382, 255]);

        let bar = LoadingBar::layout([1281, 721], 0, 361);
        // 721 * 956 / 503 = 1370.3
        assert_eq!(bar.background, [-44, 0, 1370, 721]);
        assert_eq!(bar.logo, [640 - 180, 18]);
        assert_eq!(bar.filled, [490, 344, 0, 30]);
        assert_eq!(bar.empty, [490, 344, 300, 30]);
    }

    /// Transcription test of client.java:1644-1659.
    #[test]
    fn the_world_list_stage_follows_the_fetch_result() {
        assert_eq!(
            WorldListStage::of(WorldListStartup::Loading),
            WorldListStage::Bar {
                percent: 95,
                text: "Loading world list data",
                done: false
            }
        );
        assert_eq!(
            WorldListStage::of(WorldListStartup::Loaded),
            WorldListStage::Bar {
                percent: 96,
                text: "Loaded world list data",
                done: true
            }
        );
        assert_eq!(
            WorldListStage::of(WorldListStartup::Full),
            WorldListStage::Fatal("worldlistfull".into())
        );
        assert_eq!(
            WorldListStage::of(WorldListStartup::Failed(1001)),
            WorldListStage::Fatal("worldlistio_1001".into())
        );
    }

    #[test]
    fn a_pack_without_the_entry_holds_the_pinned_cache() {
        let pack = Pack::lazy(
            "test".into(),
            vec![],
            vec![],
            Box::new(|name: &str| Err(format!("{name} absent"))),
        );
        assert_eq!(Assets::read(&pack).unwrap(), Assets::PINNED);
        let entry = br#"{"loginscreen":7,"scape main":8,"titlebg":9,"logo":10}"#;
        let pack = Pack::lazy(
            "test".into(),
            vec![],
            vec![(Assets::ENTRY.to_owned(), entry.to_vec())],
            Box::new(|name: &str| Err(format!("{name} absent"))),
        );
        assert_eq!(
            Assets::read(&pack).unwrap(),
            Assets {
                interface: 7,
                song: 8,
                background: 9,
                logo: 10
            }
        );
        let pack = Pack::lazy(
            "test".into(),
            vec![],
            vec![(Assets::ENTRY.to_owned(), br#"{"logo":10}"#.to_vec())],
            Box::new(|name: &str| Err(format!("{name} absent"))),
        );
        assert!(Assets::read(&pack).is_err());
    }
}
