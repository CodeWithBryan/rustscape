//! Which `mapdots` frame `MiniMap.render` draws for a player.

/// `player.username.encode37()` for a player whose appearance carried `name37`.
///
/// `Player.decodeAppearance` stores `Base37.decode37(name37).toTitleCase()`.
/// Every key `decode37` accepts ends in a non-zero digit and is shorter than
/// thirteen digits, so encoding the title-cased text gives the key back.
/// `decode37` returns null for the rest and the original then fails inside
/// `decodeAppearance`; None stands for that player never existing.
pub fn username_key(name37: i64) -> Option<i64> {
    (name37 > 0 && name37 < 6_582_952_005_840_035_281 && name37 % 37 != 0).then_some(name37)
}

/// The dot of one visible player: an online friend (frame 3) before a clan
/// member (5) before a team mate (4) before anyone else (2).
///
/// `friends` are `FriendsList.encodedUsernames` with `worlds`; `clan` the
/// first `ClanChat.size` member keys.
pub fn player_dot(
    name: i64,
    friends: impl IntoIterator<Item = (i64, i32)>,
    clan: impl IntoIterator<Item = i64>,
    own_team: i32,
    team: i32,
) -> usize {
    let friend = friends
        .into_iter()
        .any(|(key, world)| name == key && world != 0);
    let clan_member = clan.into_iter().any(|key| name == key);
    let team_mate = own_team != 0 && team != 0 && team == own_team;
    if friend {
        3
    } else if clan_member {
        5
    } else if team_mate {
        4
    } else {
        2
    }
}
