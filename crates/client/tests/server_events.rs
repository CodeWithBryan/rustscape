//! World-side effects of the server messages decoded by `ServerEvent`.
//!
//! Expected values are worked out from the pinned RT4 sources named below.
//! These are transcription tests: they pin the reference files they were
//! written against, but they do not execute the original classes.
use rs09_client::{
    actor_animation::State as Animation,
    animation::Sequence,
    world_spot::{Context, State},
};
use rs09_protocol::game::events::HalfTileProjectile;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// SHA-256 of each transcribed reference file with CR removed.
const SOURCES: &[(&str, &str)] = &[
    // Component key bindings, CS2 5100 to 5102 and 6600/6601, and where the
    // input classes are called from (`input`, `ui::vm_ops`).
    (
        "InterfaceList.java",
        "bba84813d0803e1cb3e632b033f6bf7b09994a46887c07982ee8f876a1a28454",
    ),
    (
        "ScriptRunner.java",
        "86e1e9553fbc7a1c7b91cbf0510a59d5d0f112f531cac5d6b77d5a99623ddbc1",
    ),
    (
        "Protocol.java",
        "65838b0ebcdbeb2ed0fb785b15e2abe5d409fceaa1b1456c33ef6cb5cb756f02",
    ),
    (
        "ServerProt.java",
        "a87d61c5c01583908b55007616fecf75cbc1b618d35476b4e9678e7dc3b3efcc",
    ),
    (
        "ClientProt.java",
        "c7c369682d1fa062af4c9cefd23701cc86e39a9c12f90f26e2402bf5c49c1315",
    ),
    (
        "FriendsList.java",
        "6d7ebadb8faa6ac00acad343fead79973eb7cfafea9ed90d9c96b17a10bbe0f7",
    ),
    (
        "IgnoreList.java",
        "693476d25d0078d2c236457df5370bc5349c2697f92bfdddc876bb6164e43a16",
    ),
    (
        "ClanChat.java",
        "ebdb2552ca82fb93a6eeb1a877bba8a3f38bc75fc835e07c10646f8f0a59c16a",
    ),
    (
        "StockMarketOffer.java",
        "32a79eef15d85a882cbbe4cfacc561fdf114cc323298d2cd4b6abbe60c74f863",
    ),
    (
        "Base37.java",
        "3a886f6df1766521b332f5f1848a38a11f1d2a896a05cc3372b70b43e6e8860c",
    ),
    (
        "JagString.java",
        "1f9aca0704ff1b983c1c800541cd252670a81d4ede63a1621edff602ed0366f5",
    ),
    (
        "Chat.java",
        "7e09c9b66ad7c470087bd7d7650989f43b36ff260cacf7be6cf9697daff310fe",
    ),
    (
        "DelayedStateChange.java",
        "78e7c96684140e72c016f978d68327b6c1b1c92dcb6211164ca58616c027366e",
    ),
    // The main loop's own packets and its clock: `rs09_client::client_events`,
    // `rs09_client::clock` and `Session::cycle`.
    (
        "client.java",
        "8e5a0c086a1e95a23f2a688ee8d6ba3e3850ab571dad9d553bb69789514abceb",
    ),
    (
        "GameShell.java",
        "e504764f0d5807a8e6515eaa78214fcd6458c30bfde0062a4413a01f0c448bb3",
    ),
    (
        "NanoTimer.java",
        "d12deaa5dbd12d54aba9644a6d2ed8ddae9fd7b1b01c6574fa2ec0af97dc7b4e",
    ),
    (
        "LoginManager.java",
        "882aefdb9fcdd3452893fe8500c194be367640da86097c4f0f20101b5707f005",
    ),
    (
        "DisplayMode.java",
        "149d4f38f63bef0f0d40f087fe1599223ee2923e0d2682033fc663700e811ea8",
    ),
    (
        "Mouse.java",
        "8d6ce7881e79779b69f3938f44ed45fdb0243bedd821390952c446b24dc5616a",
    ),
    (
        "Keyboard.java",
        "e1ae831ac0a1fa47766ea2d097fd471891c8931e57b239d63298f2079beacfee",
    ),
    (
        "MouseRecorder.java",
        "fb65870f936b5c674a2205b07e21697f39ef79a9eb2c1032164ae0684233b207",
    ),
    // The context menu's open state, the hover action text and cursors
    // (`context_menu`, `ui::action_tooltip`, `cursor`).
    (
        "MiniMenu.java",
        "53bea8e86338848348ae62d62b0aee9b2ddd4d0f8f15e569319c1b55e8d5f804",
    ),
    (
        "Font.java",
        "9c221bab222faab302a86b73006648f3cfffdfee2a02cec298e5418f1230569d",
    ),
    (
        "Cs1ScriptRunner.java",
        "3a3839925de1deccc3edbca1eafc3b59417acbe24e9ba4add1ef80875d3a1bf9",
    ),
    (
        "CursorType.java",
        "188b21b206338dc8c8675337a3f0f9847a7690fbb3762fd56daf671532a75785",
    ),
    (
        "CursorTypeList.java",
        "4a290e4243f7910838ea07bb6f32cb10efae84ec4def13f73f62db88c03e8632",
    ),
    (
        "SoftwareSprite.java",
        "ca31019dbae7b2e4d3866d29d7a072b00a60794ec03a3326101543d8671c90e7",
    ),
    (
        "ArrayUtils.java",
        "c9fbdbb9632ef900366df75723443b9706cd7b7a12fee3f3e2c32a72fe6b4aab",
    ),
    (
        "LocalizedText.java",
        "3128ce054ad84a17e195e240ab494eaefe14caa3190444e58d0325f37f475267",
    ),
    // Minimap drift, player dots, labels and raster rebuild.
    (
        "MiniMap.java",
        "a179ec6a1ba55d7d66a6f5e71e4107a7839205952afaa9544bc439a66adf217c",
    ),
    (
        "SceneGraph.java",
        "eec94c9c65b1044f10f9e3c0cf9bc06c37f7d398f3ac8488bba5f68c26340166",
    ),
    (
        "MapElementList.java",
        "febf3955d1118ab5e4b4e0a1e39212941fb924e0a3e18edaf6396df861c4c6c9",
    ),
    (
        "Camera.java",
        "28461c2ee97afb5886ec314796266cd1ddec74d799fb34de0247f48967f9bf17",
    ),
    (
        "Player.java",
        "3b92fb4c4fd38e8796f2d309f0da0a301713f35b2bc22afed52c9471676998ed",
    ),
    // The follow camera, projection and scene viewport.
    (
        "GlRenderer.java",
        "d314e4167e39e36166a348fdcd6cb874cc8346304f34bc9c44835dc09b0f3858",
    ),
    (
        "MathUtils.java",
        "de35c8e8b133b37b8376f2ad0cfb19565e0e7c4c1ff14a772d64d0b9db53436b",
    ),
    (
        "GlobalConfig.java",
        "9aea9139a1f866fdfbfb2bd71277579b9c0540a525b62db48c066da3f95a7a67",
    ),
    // Roof removal (`roof`).
    (
        "LocType.java",
        "ee83bf276c98ee020295873ea9e56424821b2070b807a3910ca7740c263f891c",
    ),
    (
        "GlTile.java",
        "a1af01c7ec0cfb7890acc405299f89174ebb1d1be44045d3781956952884efa6",
    ),
    // Water and the special materials (`materials`).
    (
        "MaterialManager.java",
        "1d894aec8f9a16918557d434440b887296a51b3f3fa57640462940e3f1f7c829",
    ),
    (
        "LiquidMaterialRenderer.java",
        "9feecf2e018626592a9a560c3af333941c768a6c1ef747d13f11eef26c896ae7",
    ),
    (
        "WaterMaterialRenderer.java",
        "3997446d23b0f106a0e16e407253bb9ff085891f92fccfd65ea63afa94c3d770",
    ),
    (
        "WaterfallMaterialRenderer.java",
        "c0767878c8c4d23afaf53ed259ff8109ed4b7bc954800ce7f370ee9fb19ee843",
    ),
    (
        "SpecularMaterialRenderer.java",
        "56ae34bd93d6f684ead41776ab5473a74dabd2b53f0676f9219093eae05e6554",
    ),
    (
        "UnlitMaterialRenderer.java",
        "23ff0bb5b25558b81c1df5af91afaa618acb8a1aab3ebba64a30e2d96f7f3481",
    ),
    (
        "UnderwaterMaterialRenderer.java",
        "41ebdbea5469958921d5a48b0759d9b290216c4c15fe93ee1915e42846a8def7",
    ),
    (
        "GlTexture.java",
        "408263deec0cb1eebe1b4e4f6277994bc04556032b9b478d3b86df07202792c7",
    ),
    // World picking and the tile walk (`picking`, `scene_order`).
    (
        "PathFinder.java",
        "549f435f91605037eb6a31edbfad46f8f30fd596062e965ec377040528ab0d5b",
    ),
    (
        "Npc.java",
        "b661a948703b7bc399a4423f8f138dcde7e6f242de79f5433f96f736c7d6419e",
    ),
    (
        "GlModel.java",
        "901a0e9e620407bb979b3bdd76d892a5d4d878c4e57369b8854fceae3b011d48",
    ),
    // Interface fidelity (hook walk, client codes, item sprites, outlines).
    (
        "Inv.java",
        "f53b0e4b588189d1eacf1088686b2780b061a667fd92f92af872180729774ea6",
    ),
    (
        "SoftwareFont.java",
        "c79cec901c96ce178060adb6b9ac54fc954ff02d846eddc11f4c4f3a05ca69e4",
    ),
    (
        "SoftwareRaster.java",
        "88eb7394e5db4cb7eedb0f455a712314f4932d7458aac85fb32b8e0fbd367dd1",
    ),
    (
        "GlRaster.java",
        "698962a9e5607cc2a85b2ce3ef6ee596caf30c2558b0c14b4a6e63fe6383cef1",
    ),
    (
        "Rasteriser.java",
        "be9451f63ebf32f40183f08e0ead2e02d705e8c4cde7feee17522f04d347115b",
    ),
    (
        "StringUtils.java",
        "820a82427d560f31c032c7d9ad69f86bc65bc0ebb4233766db58cf5567875ba6",
    ),
    (
        "Cheat.java",
        "ffe80e81d15de204702eccd9feb9ba7e4ae6e31d5afc93ac12fcc01a193bcca2",
    ),
    // Actors (hint arrows, NPC palettes, the previous-body fallback); the
    // other files that work transcribes from are listed above, and all of
    // them are also pinned in tests/actors_oracle.rs.
    (
        "HintArrowManager.java",
        "61a7464a1234760d66880c89d7bf600943ab5890ac4a5ac2a3e4c8ae59642cc2",
    ),
    (
        "NpcType.java",
        "a13acf5f344708fda80d4ce34cfcffa25c5fb41ab21036f26771865e639a723b",
    ),
    (
        "PlayerAppearance.java",
        "c49a5f3fb187e90b9d60e15f1495416732361fce55b91101612ff59f100f5a0d",
    ),
    // The loading bar and title screen (`title`).
    (
        "LoadingBar.java",
        "213260eee717b16c7daf35b19adccdfaec53fabaac6f532066260893da6c9e7a",
    ),
    (
        "TitleScreen.java",
        "cf6018bab056e3573b7f07083a62cf6fe9ca463eeef9491c1d8942adf6cdb8d4",
    ),
    // Location placement, terrain mesh and lighting.
    (
        "Loc.java",
        "0fcfb4d6387982a745305cd317566bc0098bc464111ead82799d01a74023555f",
    ),
    (
        "FluType.java",
        "b70c1f91ce92bd641698c5cdd3dc9d7125ee106314c929078ee6ab6a769632ae",
    ),
    (
        "FloType.java",
        "0d39a85e0b2dfc5b65efc74e383a90d383a9559c06472e01326bc7b5bd705a49",
    ),
    // The underwater scene's backdrop colour and the frame's fog state
    // (`underwater`).
    (
        "ColorUtils.java",
        "d0c000bf36313b5fb2046267176c03ab5328c5714186ce2819cd55ed8ecf296d",
    ),
    (
        "FogManager.java",
        "3f088f20fd85e91c51749cba00b8adcca4b2dfb1119fc480a38eff447ebee180",
    ),
    // The title scene (`title_scene`).
    (
        "CollisionMap.java",
        "dae85a8d9acd111bcd46a5c3bfb5533e87fdfabdbaf4b805cb8b45856ebb5098",
    ),
    (
        "PathingEntity.java",
        "e7314bad5782f62c1c6800948bac93a3af9a96c2ce8de398862edada6b830da0",
    ),
    // Standard-detail lighting and low-detail textures (`sd_light`,
    // `texture::low_detail`): what the two preferences switch off or shrink.
    (
        "LightingManager.java",
        "0f3112324d6e7e220c799cdc9fd49731963f7fad1be4dc43b6d7a9ee293cc1f9",
    ),
    (
        "Js5GlTextureProvider.java",
        "cf65fea9fac1c3cb4a7a63ec349a80d7446ecec5ebf82fb5b9b91995a0c40055",
    ),
    // Depth layers and occluders (`depth_layers`, `occlusion`).
    (
        "Occluder.java",
        "dc0f4fb91e7bdd8af2f8a0657eef87a5d7e2211289b9e1b40e9ee060c403867a",
    ),
    (
        "LightingManager.java",
        "0f3112324d6e7e220c799cdc9fd49731963f7fad1be4dc43b6d7a9ee293cc1f9",
    ),
    (
        "ShadowManager.java",
        "45c51ce9bc09a12b90c193f9498ddf3ce319abde12272bd4ce2c67c773ddff3f",
    ),
    // Types read on demand and their defaults (`on_demand`, `npc_types`,
    // `actor_motion::cancel_on_move`), and the start gate (`cache_update`).
    (
        "NpcTypeList.java",
        "d13defd9ad8f5e80503c173af4a17aec734139ba51065b09329a93458c359666",
    ),
    (
        "SeqType.java",
        "2c0cbbae8b719619385474532fb999a6aec21052a2c99cd29af86232c3a64cb3",
    ),
    (
        "SeqTypeList.java",
        "f94b75d4c2de791d3a30cdc3147e4954ca839f849d85dbc256d419c1a775adb5",
    ),
    (
        "Js5.java",
        "b6c3449c5138544397539b9e052bb742be9fbdf3db34356d1f129036b786e07e",
    ),
];

#[test]
fn transcribed_reference_sources_are_the_pinned_ones() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    for &(name, expected) in SOURCES {
        let text = String::from_utf8(std::fs::read(root.join(name)).unwrap())
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected,
            "{name} changed: review the server-event transcription"
        );
    }
}

/// Further reference files transcribed for the server-message semantics
/// (game message requests, PLAYER_INFO chat, zone envelope children, the
/// reboot countdown, varc and varp resets, rebuild side effects, ground
/// option 2 and the run key), pinned the same way. `Protocol.java`,
/// `ClientProt.java`, `JagString.java` and `Chat.java` are in `SOURCES`.
const SEMANTICS_SOURCES: [(&str, &str); 9] = [
    (
        "VarpDomain.java",
        "cf74c00bf58c026436e00ca04221ba93ccc41cde27a3483b832ff68ebc18b669",
    ),
    (
        "VarpTypeList.java",
        "09e1a4e83a142ceadc7877bb13dcb07d3e8b8a87b4bb3c3bc54754da6ffaec67",
    ),
    (
        "Js5Index.java",
        "2f39eedba0969e61a80ce5bdc3571deaaf28d1fd45b0c0edacd095159d3e7c03",
    ),
    (
        "LoginManager.java",
        "882aefdb9fcdd3452893fe8500c194be367640da86097c4f0f20101b5707f005",
    ),
    (
        "client.java",
        "8e5a0c086a1e95a23f2a688ee8d6ba3e3850ab571dad9d553bb69789514abceb",
    ),
    (
        "SceneGraph.java",
        "eec94c9c65b1044f10f9e3c0cf9bc06c37f7d398f3ac8488bba5f68c26340166",
    ),
    (
        "MiniMenu.java",
        "53bea8e86338848348ae62d62b0aee9b2ddd4d0f8f15e569319c1b55e8d5f804",
    ),
    (
        "QuickChatPhraseType.java",
        "5f3cdd6c276752bdd39fc233284daf58cad0aefb16bdf0b92153abac70c17e5a",
    ),
    (
        "LocalizedText.java",
        "3128ce054ad84a17e195e240ab494eaefe14caa3190444e58d0325f37f475267",
    ),
];

#[test]
fn transcribed_semantics_sources_are_the_pinned_ones() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    for (name, expected) in SEMANTICS_SOURCES {
        let text = String::from_utf8(std::fs::read(root.join(name)).unwrap())
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected,
            "{name} changed: review the server-message semantics transcription"
        );
    }
}

/// The files the world map's interface wiring was transcribed from
/// (`ui/world_map_host.rs`, `ui/vm_world_map.rs`), besides `Protocol.java`,
/// `ClientProt.java` and `JagString.java` above.
const WORLD_MAP_SOURCES: [(&str, &str); 8] = [
    (
        "ScriptRunner.java",
        "86e1e9553fbc7a1c7b91cbf0510a59d5d0f112f531cac5d6b77d5a99623ddbc1",
    ),
    (
        "InterfaceList.java",
        "bba84813d0803e1cb3e632b033f6bf7b09994a46887c07982ee8f876a1a28454",
    ),
    (
        "Cs1ScriptRunner.java",
        "3a3839925de1deccc3edbca1eafc3b59417acbe24e9ba4add1ef80875d3a1bf9",
    ),
    (
        "WorldMap.java",
        "89eee2800c3b08e71720690afcba945a6b3f1704c507206d6ee2de27cc6c1b35",
    ),
    (
        "MapList.java",
        "f251f980d6ab1523c4ba8dfbf8df1483db991033543a51623419ddabc4f764bf",
    ),
    (
        "Cheat.java",
        "ffe80e81d15de204702eccd9feb9ba7e4ae6e31d5afc93ac12fcc01a193bcca2",
    ),
    (
        "client.java",
        "8e5a0c086a1e95a23f2a688ee8d6ba3e3850ab571dad9d553bb69789514abceb",
    ),
    (
        "LoginManager.java",
        "882aefdb9fcdd3452893fe8500c194be367640da86097c4f0f20101b5707f005",
    ),
];

#[test]
fn world_map_wiring_sources_are_the_pinned_ones() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    for (name, expected) in WORLD_MAP_SOURCES {
        let text = String::from_utf8(std::fs::read(root.join(name)).unwrap())
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected,
            "{name} changed: review the world map wiring"
        );
    }
}

/// SHA-256, with CR removed, of the reference files the session lifecycle
/// was transcribed from (`src/lifecycle.rs`, `src/ui/vm_session.rs`,
/// `src/ui/state_session.rs`, `tests/lifecycle.rs`). `Protocol.java` and
/// `ServerProt.java` are pinned above.
const LIFECYCLE_SOURCES: [(&str, &str); 12] = [
    (
        "client.java",
        "8e5a0c086a1e95a23f2a688ee8d6ba3e3850ab571dad9d553bb69789514abceb",
    ),
    (
        "LoginManager.java",
        "882aefdb9fcdd3452893fe8500c194be367640da86097c4f0f20101b5707f005",
    ),
    (
        "InterfaceList.java",
        "bba84813d0803e1cb3e632b033f6bf7b09994a46887c07982ee8f876a1a28454",
    ),
    (
        "Fonts.java",
        "57fc9a42299c7df505dcbdbff67b9a6d6f265eca75576f27dceaa2d6a3ccfeb2",
    ),
    (
        "Font.java",
        "9c221bab222faab302a86b73006648f3cfffdfee2a02cec298e5418f1230569d",
    ),
    (
        "VarpDomain.java",
        "cf74c00bf58c026436e00ca04221ba93ccc41cde27a3483b832ff68ebc18b669",
    ),
    (
        "VarcDomain.java",
        "cf3ce6065a8b38978fa42120f12acdaf3adc0bf8cf75d8d0e3deed2bc4447d9a",
    ),
    (
        "Camera.java",
        "28461c2ee97afb5886ec314796266cd1ddec74d799fb34de0247f48967f9bf17",
    ),
    (
        "LocalizedText.java",
        "3128ce054ad84a17e195e240ab494eaefe14caa3190444e58d0325f37f475267",
    ),
    (
        "TracingException.java",
        "26a3bb228bd9809a17ed3a7bc1cc5493c855dbb029de661e2bf2957698185a8e",
    ),
    (
        "ScriptRunner.java",
        "86e1e9553fbc7a1c7b91cbf0510a59d5d0f112f531cac5d6b77d5a99623ddbc1",
    ),
    (
        "Player.java",
        "3b92fb4c4fd38e8796f2d309f0da0a301713f35b2bc22afed52c9471676998ed",
    ),
];

#[test]
fn lifecycle_reference_sources_are_the_pinned_ones() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    for (name, expected) in LIFECYCLE_SOURCES {
        let text = String::from_utf8(std::fs::read(root.join(name)).unwrap())
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            expected,
            "{name} changed: review the session lifecycle transcription"
        );
    }
}

fn projectile(source: Option<i32>) -> HalfTileProjectile {
    HalfTileProjectile {
        offset: [5, 9],
        delta: [-3, 4],
        target: -12,
        source,
        spot: 90,
        start_offset: 172,
        end_height: 30,
        start: 5,
        end: 40,
        angle: 16,
        distance: 64,
    }
}
fn context() -> Context {
    Context {
        origin: [3200, 3400],
        plane: 1,
        cycle: 1000,
    }
}

#[test]
fn half_tile_projectile_uses_twice_the_zone_origin_and_sixty_four_unit_steps() {
    let mut state = State {
        zone: [16, 24],
        ..State::default()
    };
    let mut sampled = vec![];
    state.launch(
        &projectile(None),
        context(),
        |_, _, _| panic!("121 has no launching actor"),
        &mut |plane, fine| {
            sampled.push((plane, fine));
            -500
        },
    );
    // (16 * 2 + 5, 24 * 2 + 9) half tiles, 64 fine units each.
    assert_eq!(sampled, [(1, [37 * 64, 57 * 64])]);
    let shot = &state.projectiles[0];
    assert_eq!(
        shot.position,
        [
            f64::from(37 * 64 + 3200 * 128),
            f64::from(57 * 64 + 3400 * 128),
            -672.0
        ]
    );
    assert_eq!(
        shot.destination,
        [34 * 64 + 3200 * 128, 61 * 64 + 3400 * 128]
    );
    assert_eq!((shot.start, shot.end, shot.plane), (1005, 1040, 1));
    assert_eq!(
        (
            shot.packet.target,
            shot.packet.spot,
            shot.packet.end_height,
            shot.packet.angle,
            shot.packet.distance
        ),
        (-12, 90, 30, 16, 64)
    );
}

#[test]
fn launching_actor_slot_moves_the_source_before_the_height_is_sampled() {
    let mut state = State {
        zone: [16, 24],
        ..State::default()
    };
    let mut asked = vec![];
    let mut sampled = vec![];
    // NPC index 7, BAS slot 3: (3 << 11 | 7) + 1.
    state.launch(
        &projectile(Some((3 << 11 | 7) + 1)),
        context(),
        |npc, index, slot| {
            asked.push((npc, index, slot));
            // Translate 100 right, 40 up and 20 forward, facing angle 512.
            Some(([100, 40, 20, 0, 0, 0], 512))
        },
        &mut |_, fine| {
            sampled.push(fine);
            0
        },
    );
    assert_eq!(asked, [(true, 7, 3)]);
    // The 2048-step table truncates sin(512) to 65535 and cos(512) to 0, so
    // x += (100 * 0 + 20 * 65535) >> 16 = 19 and
    // y += (0 * 20 - 100 * 65535) >> 16 = -100 (an arithmetic shift).
    assert_eq!(sampled, [[37 * 64 + 19, 57 * 64 - 100]]);
    // The slot's vertical translation is subtracted from the launch offset.
    assert_eq!(state.projectiles[0].position[2], -(172.0 - 40.0));

    // A negative ID addresses a player; an unresolved actor leaves the source.
    let mut asked = vec![];
    state.launch(
        &projectile(Some(-((2 << 11 | 900) + 1))),
        context(),
        |npc, index, slot| {
            asked.push((npc, index, slot));
            None
        },
        &mut |_, fine| {
            assert_eq!(fine, [37 * 64, 57 * 64]);
            0
        },
    );
    assert_eq!(asked, [(false, 900, 2)]);
    assert_eq!(state.projectiles[1].position[2], -172.0);

    // Zero means no launching actor at all.
    state.launch(
        &projectile(Some(0)),
        context(),
        |_, _, _| panic!("zero does not name an actor"),
        &mut |_, _| 0,
    );
    assert_eq!(state.projectiles.len(), 3);
}

#[test]
fn projectiles_outside_the_doubled_scene_or_without_a_graphic_are_dropped() {
    let launch = |zone: [i32; 2], packet: HalfTileProjectile| {
        let mut state = State {
            zone,
            ..State::default()
        };
        state.launch(&packet, context(), |_, _, _| None, &mut |_, _| 0);
        state.projectiles.len()
    };
    assert_eq!(launch([16, 24], projectile(None)), 1);
    // Half-tile coordinates run to 207, twice the 104-tile scene.
    assert_eq!(launch([101, 24], projectile(None)), 1);
    assert_eq!(launch([102, 24], projectile(None)), 0);
    assert_eq!(launch([16, 100], projectile(None)), 0);
    // The source is inside but the destination (delta -3) is not.
    assert_eq!(launch([-2, 24], projectile(None)), 0);
    assert_eq!(launch([-1, 24], projectile(None)), 1);
    assert_eq!(
        launch(
            [16, 24],
            HalfTileProjectile {
                spot: 65535,
                ..projectile(None)
            }
        ),
        0
    );
}

fn sequences() -> BTreeMap<u32, Sequence> {
    let sequence = |priority, sounds: Vec<Vec<u32>>| Sequence {
        frames: vec![0, 1],
        delays: vec![1, 1],
        sounds,
        priority,
        ..Default::default()
    };
    BTreeMap::from([
        (10, sequence(5, vec![vec![7 << 8]])),
        (11, sequence(3, vec![])),
        (12, sequence(8, vec![])),
    ])
}

#[test]
fn entity_spot_keeps_unsigned_height_and_the_priority_rule() {
    let spots = BTreeMap::from([(100, 10), (101, 11), (102, 12)]);
    let sequences = sequences();
    let mut state = Animation::default();
    // A height above 32767 survives; the packed mask form would sign-extend it.
    let sounds = state
        .request_spot_at(100, 40000, 0, 500, &spots, &sequences)
        .unwrap();
    assert_eq!(
        (state.spot.id, state.spot_height, state.spot_start),
        (100, 40000, 500)
    );
    assert_eq!(state.spot.frame, 0);
    assert_eq!(sounds.len(), 1, "a spot starting this cycle plays frame 0");

    // A lower-priority graphic does not replace the running one.
    state
        .request_spot_at(101, 10, 0, 501, &spots, &sequences)
        .unwrap();
    assert_eq!((state.spot.id, state.spot_height), (100, 40000));
    // An equal or higher priority does, and a delayed start waits at frame -1.
    let sounds = state
        .request_spot_at(102, 20, 30, 501, &spots, &sequences)
        .unwrap();
    assert_eq!(
        (
            state.spot.id,
            state.spot_height,
            state.spot_start,
            state.spot.frame
        ),
        (102, 20, 531, -1)
    );
    assert!(sounds.is_empty());
    // Clearing always applies.
    state
        .request_spot_at(-1, 0, 0, 502, &spots, &sequences)
        .unwrap();
    assert_eq!(state.spot.id, -1);
    // The mask form is the same request with both fields in one word.
    let mut packed = Animation::default();
    packed
        .request_spot(102, (20 << 16) | 30, 501, &spots, &sequences)
        .unwrap();
    assert_eq!(
        (packed.spot.id, packed.spot_height, packed.spot_start),
        (102, 20, 531)
    );
    assert!(
        state
            .request_spot_at(999, 0, 0, 502, &spots, &sequences)
            .is_err(),
        "an unknown graphic is reported, not silently accepted"
    );
}

#[test]
fn reset_animations_clears_only_the_action_sequence() {
    let sequences = sequences();
    let mut state = Animation::default();
    state.request(12, 4, 2, &sequences).unwrap();
    let before = state.action.clone();
    state.reset_action();
    assert_eq!(state.action.id, -1);
    assert_eq!(
        (state.action.frame, state.action.delay, state.movement_steps),
        (before.frame, before.delay, 2)
    );
    // With the action cleared, a low-priority request is accepted again.
    state.request(11, 0, 0, &sequences).unwrap();
    assert_eq!(state.action.id, 11);
}
