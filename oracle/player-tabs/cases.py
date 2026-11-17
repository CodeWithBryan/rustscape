"""The designed player tab corpus: every case is (case, sweep), where sweep names the
operation whose recorded calls each fail once in turn (None for no sweep)."""
import pathlib
import random
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]

NEEDED_EVENTS = ['ticks', 'duration', 'animate', 'item_config', 'equipment', 'equipment_new', 'spell_lookup', 'spell_book',
                 'update_style', 'is_attacking', 'attack_style', 'set_attack_style', 'set_attack_animation', 'autocast', 'set_autocast',
                 'combat_level', 'style_index', 'toggle_style_index', 'special_toggled', 'toggle_special_bar', 'is_npc', 'tabs',
                 'resizable', 'open_tab', 'open_chatbox', 'interface_config', 'message', 'string', 'access_mask', 'graphic', 'animation',
                 'interface', 'music', 'static_level', 'prayer_points', 'total_level', 'combat_milestone', 'set_combat_milestone',
                 'skill_milestone', 'set_skill_milestone', 'skillcape_trim', 'news', 'is_locked', 'lock', 'lock_all', 'diary_complete',
                 'diary_update', 'dialogue', 'pulse_clear', 'set_attribute', 'get_attribute', 'remove_attribute', 'add_extension',
                 'is_debug', 'debug', 'rights', 'in_combat', 'varp', 'varbit', 'log', 'custom_config', 'update_varps']

# The Quests enum, in declaration order, as the case rows carry it (Support checks each row against the compiled enum).
QUESTS_SOURCE = ROOT / 'reference/server/Server/src/main/content/data/Quests.kt'
QUESTS = re.findall(r'^\s+([A-Z0-9_]+)\("((?:[^"\\]|\\.)*)"\)[,;]', QUESTS_SOURCE.read_text(encoding='utf-8'), re.M)

EMOTES = ['YES', 'NO', 'BOW', 'ANGRY', 'THINK', 'WAVE', 'SHRUG', 'CHEER', 'BECKON', 'JUMP_FOR_JOY', 'LAUGH', 'YAWN', 'DANCE', 'JIG',
          'SPIN', 'HEADBANG', 'CRY', 'BLOW_KISS', 'PANIC', 'RASPBERRY', 'CLAP', 'SALUTE', 'GOBLIN_BOW', 'GOBLIN_SALUTE', 'GLASS_BOX',
          'CLIMB_ROPE', 'LEAN', 'GLASS_WALL', 'IDEA', 'STOMP', 'FLAP', 'SLAP_HEAD', 'ZOMBIE_WALK', 'ZOMBIE_DANCE', 'ZOMBIE_HAND',
          'SCARED', 'BUNNY_HOP', 'SKILLCAPE', 'SNOWMAN_DANCE', 'AIR_GUITAR', 'SAFETY_FIRST', 'EXPLORE', 'TRICK', 'FREEZE', 'GIVE_THANKS']
# Content overrides (diaries, quests, dialogue, a world pulse) the corpus does not drive through play.
CONTENT_EMOTES = {'SALUTE', 'GOBLIN_BOW', 'GIVE_THANKS'}
CONTENT_BUTTONS = {23, 24, 46}

INTERFACES = ['UNARMED', 'STAFF', 'AXE', 'SCEPTER', 'PICKAXE', 'SWORD_DAGGER', 'SCIMITAR', 'TWO_H_SWORD', 'MACE', 'CLAWS',
              'WARHAMMER_MAUL', 'WHIP', 'FLOWERS', 'MUD_PIE', 'SPEAR', 'HALBERD', 'BOW', 'CROSSBOW', 'THROWN_WEAPONS', 'CHINCHOMPA',
              'FIXED_DEVICE', 'SALAMANDER', 'SCYTHE', 'IVANDIS_FLAIL']
STYLES = [[0, 2], [1, 2], [3, 2], [7, 3], [8, 3], [0, 1], [2, 1], [4, 4], [5, 4], [6, 4], [1, 0], [2, 0], [3, 0]]
DEFINITIONS = {'92': ['TAB', True, 0], '319': ['DEFAULT', False, -1], '740': ['DIALOGUE', False, -1]}
MODERN_IDS = [1, 4, 6, 8, 10, 14, 17, 20, 24, 27, 33, 38, 45, 48, 52, 55, 22, 31, 42]
ANCIENT_IDS = list(range(20))
REGISTERED = {'MODERN': MODERN_IDS, 'ANCIENT': ANCIENT_IDS}

STAFF, SLAYER, MACE, ANCIENT, ZURIEL = 1381, 4170, 8841, 4675, 13867


def anims(*ids, priority='HIGH'):
    return [[i, 0, priority] for i in ids]


def weapon_items():
    items = {
        str(STAFF): dict(name='Staff of air', weapon_interface=1, has_special=False, attack_anims=anims(419, 419, 419, 419, 419)),
        str(SLAYER): dict(name="Slayer's staff", weapon_interface=1, attack_anims=anims(419, 419)),
        str(MACE): dict(name='Void knight mace', weapon_interface=1, has_special=True),
        str(ANCIENT): dict(name='Ancient staff', weapon_interface=1, attack_anims=anims(419, 419, 419, 419, 419)),
        str(ZURIEL): dict(name="Zuriel's staff", weapon_interface=1, attack_anims=anims(419, 419, 419, 419, 419)),
        '995': dict(name='Coins'),
        '30000': dict(name='Broken', weapon_interface=24),
    }
    for k, name in enumerate(INTERFACES):
        entry = dict(name=name.title().replace('_', ' '), weapon_interface=k)
        if k % 2 == 0:
            entry['has_special'] = k % 4 == 0
        if k % 3 == 0:
            entry['attack_anims'] = anims(*[400 + k * 4 + i for i in range(4)])
        elif k % 3 == 1:
            entry['attack_anims'] = anims(400 + k, 401 + k, priority='MID')
        items[str(20000 + k)] = entry
    return items


ITEMS = weapon_items()
# The item and spell tables every case shares (Support reads them from tables.json unless a case replaces them).
TABLES = dict(items=ITEMS, registered=REGISTERED, quests=[list(q) for q in QUESTS])

# MusicEntry.SONGS of the music cases: id, name, list index. 300 is unnamed (" "), 82 and 83 share an index, 640+ are past the
# twenty configs.
SONGS = [[76, 'Harmony', 0], [62, 'Newbie Melody', 1], [2, 'Autumn Voyage', 2], [3, 'Unknown Land', 31], [4, 'Sea Shanty', 32],
         [5, 'Flute Salad', 63], [300, ' ', 5], [82, 'Book of Spells', 100], [83, 'Dream', 100], [90, 'Edge', 639], [91, 'Far', 640],
         [92, 'Beyond', 700]]


def player(**extra):
    p = dict(username='tracer')
    p.update(extra)
    return p


def case(group, ops, p=None, **extra):
    c = dict(group=group, player=p if p is not None else player(), definitions=DEFINITIONS, ops=ops)
    c.update(extra)
    return c


def groups(cases):
    out = {}
    for c in cases:
        out[c['group']] = out.get(c['group'], 0) + 1
    return dict(sorted(out.items()))


# ------------------------------------------------------------------ WeaponInterface

def weapon_cases():
    rng = random.Random(530)
    worn_choices = [None] + [20000 + k for k in range(24)]
    # Opening each interface from each saved style index and attack style.
    for k, worn in enumerate(worn_choices):
        for index in [0, 1, 2, 3, 4, 5]:
            for style in [None, STYLES[(k + index) % len(STYLES)], [3, 2]]:
                p = player(worn={'3': worn} if worn else {}, styleIndex=index, attackStyle=style)
                ops = [dict(op='weapon_new'), dict(op='weapon_open'), dict(op='weapon_update')]
                sweep = 1 if index == 0 and style is None else None
                yield case('weapon_open', ops, p), sweep
    # Every button on every interface, then a refresh after a swap.
    for k, worn in enumerate(worn_choices):
        ops = [dict(op='weapon_new'), dict(op='weapon_open')]
        ops += [dict(op='weapon_set_style', button=b) for b in range(-1, 8)]
        ops += [dict(op='set', fields=dict(worn={'3': worn_choices[(k + 5) % len(worn_choices)]})), dict(op='weapon_update')]
        yield case('weapon_buttons', ops, player(worn={'3': worn} if worn else {})), (3 if k % 6 == 0 else None)
    # The special bar, an NPC appearance, a toggled special bar, resizable mode.
    for k in range(0, 24, 3):
        for npc in [False, True]:
            for toggled in [False, True]:
                p = player(worn={'3': 20000 + k}, npc=npc, specialToggled=toggled, resizable=(k % 2 == 0))
                yield case('weapon_flags', [dict(op='weapon_new'), dict(op='weapon_open'), dict(op='weapon_set_style', button=2),
                                            dict(op='weapon_update')], p), None
    # Before any update: no interface and no animations.
    yield case('weapon_unset', [dict(op='weapon_new'), dict(op='weapon_set_style', button=1)], player()), None
    yield case('weapon_unset', [dict(op='weapon_new'), dict(op='weapon_can_autocast', message=True),
                                dict(op='weapon_restore_style', index=0)], player()), None
    yield case('weapon_unset', [dict(op='weapon_new'), dict(op='weapon_open_select'), dict(op='weapon_restore_autocast')], player()), None
    # A broken weapon_interface value and a weapon without one.
    yield case('weapon_broken', [dict(op='weapon_new'), dict(op='weapon_open')], player(worn={'3': 30000})), None
    yield case('weapon_broken', [dict(op='weapon_new'), dict(op='weapon_open')], player(worn={'3': 995})), 1
    yield case('weapon_static', [dict(op='weapon_interface_for', item=i) for i in [None, 995, STAFF] + [20000 + k for k in range(24)]] +
               [dict(op='weapon_interface_for', item=30000)], player()), None
    # A component definition 92 of another type, and a missing type.
    for definition in [['DEFAULT', False, -1], ['OVERLAY', True, 3], [None, False, 0]]:
        defs = dict(DEFINITIONS, **{'92': definition})
        for resizable in [False, True]:
            yield dict(case('weapon_definition', [dict(op='weapon_new'), dict(op='weapon_open')], player(resizable=resizable)),
                       definitions=defs), None
    # Staves and autocast.
    staves = [STAFF, SLAYER, MACE, ANCIENT, ZURIEL]
    for staff in staves:
        for book in [192, 193, 430]:
            for index in [0, 3, 4]:
                for autocast in [None, 1, 8, 22, 45]:
                    p = player(worn={'3': staff}, book=book, styleIndex=index, autocast=autocast)
                    ops = [dict(op='weapon_new'), dict(op='weapon_open')]
                    ops += [dict(op='weapon_autospell_id', spellId=s) for s in [1, 8, 22, 31, 42, 55, 19, 99]]
                    ops += [dict(op='weapon_can_autocast', message=True), dict(op='weapon_can_autocast', message=False)]
                    for button in [-1, 0, 1, 2, 5, 15, 16, 17, 18, 19, 20]:
                        ops.append(dict(op='weapon_select', button=button, adjust=button % 2 == 0, forget=button % 3 == 0))
                    ops += [dict(op='weapon_set_style', button=b) for b in [1, 2, 3, 4, 5, 6]]
                    sweep = 2 + 8 + 2 + 4 if (book == 192 and index == 3 and autocast == 1) else None
                    yield case('weapon_autocast', ops, p), sweep
    # Autocast selection, the select tab and restoring a saved autocast.
    for staff in staves:
        for book in [192, 193, 430]:
            for selecting in [False, True]:
                for saved in [None, (0, 3), (3, 4), (-1, 3), (5, -1), (19, 4)]:
                    attributes = {'autocast_select': selecting} if selecting else {}
                    if saved:
                        attributes['autocast:buttonid'] = saved[0]
                        attributes['autocast:style'] = saved[1]
                    p = player(worn={'3': staff}, book=book, attributes=attributes, styleIndex=3)
                    ops = [dict(op='weapon_new'), dict(op='weapon_open'), dict(op='weapon_open_select'),
                           dict(op='weapon_restore_autocast'), dict(op='weapon_update'), dict(op='weapon_restore_style', index=4),
                           dict(op='weapon_restore_style', index=9), dict(op='weapon_restore_style', index=-1)]
                    sweep = 3 if (selecting and saved == (0, 3) and book == 192) else None
                    yield case('weapon_restore', ops, p), sweep
    # A spell lookup that is not a combat spell, and an unregistered spell.
    plain = dict(REGISTERED, **{'MODERN': [1], 'MODERN:plain': [4]})
    yield case('weapon_cast', [dict(op='weapon_new'), dict(op='weapon_open'), dict(op='weapon_select', button=1, adjust=False, forget=False),
                               dict(op='weapon_select', button=2, adjust=False, forget=True),
                               dict(op='weapon_select', button=0, adjust=True, forget=False)],
               player(worn={'3': STAFF}, registered=plain)), None
    # Random sequences over the weapon operations.
    for k in range(120):
        worn = rng.choice(worn_choices + staves + staves)
        p = player(worn={'3': worn} if worn else {}, book=rng.choice([192, 192, 193, 430]), styleIndex=rng.randrange(6),
                   attackStyle=rng.choice([None] + STYLES), autocast=rng.choice([None, None, 1, 4, 8, 13]), specialToggled=rng.random() < 0.3,
                   npc=rng.random() < 0.1, resizable=rng.random() < 0.5,
                   attributes={'autocast_select': True} if rng.random() < 0.3 else {})
        ops = [dict(op='weapon_new'), dict(op='weapon_open')]
        for _ in range(12):
            kind = rng.randrange(7)
            if kind == 0:
                ops.append(dict(op='weapon_set_style', button=rng.randrange(-1, 8)))
            elif kind == 1:
                ops.append(dict(op='weapon_select', button=rng.randrange(-2, 21), adjust=rng.random() < 0.5, forget=rng.random() < 0.5))
            elif kind == 2:
                ops.append(dict(op='set', fields=dict(worn={'3': rng.choice(worn_choices + staves)})))
                ops.append(dict(op='weapon_update'))
            elif kind == 3:
                ops.append(dict(op='weapon_open_select'))
            elif kind == 4:
                ops.append(dict(op='weapon_restore_autocast'))
            elif kind == 5:
                ops.append(dict(op='set', fields=dict(book=rng.choice([192, 193, 430]))))
                ops.append(dict(op='weapon_update'))
            else:
                ops.append(dict(op='weapon_open'))
        yield case('weapon_random', ops, p), (len(ops) - 1 if k % 10 == 0 else None)


# ------------------------------------------------------------------ LevelUp

def level_cases():
    rng = random.Random(99)
    # getSkillJingle over every slot and level.
    for slot in range(25):
        ops = []
        for level in range(0, 101):
            ops.append(dict(op='set', fields=dict(levels={str(slot): level})))
            ops.append(dict(op='jingle', slot=slot))
        yield case('jingle', ops, player()), None
    # Milestones around each threshold.
    for combat in [1, 2, 3, 4, 5, 9, 10, 25, 74, 75, 126, 137, 138, 139]:
        for total in [0, 1, 49, 50, 74, 75, 99, 100, 2376]:
            for milestones in [(0, 0), (3, 1), (12, 3), (20, 9)]:
                p = player(combatLevel=combat, totalLevel=total, combatMilestone=milestones[0], skillMilestone=milestones[1])
                yield case('milestones', [dict(op='milestones', slot=(combat + total) % 24, amount=1)], p), None
    # The level up itself.
    for slot in range(24):
        for level in [2, 50, 98, 99]:
            for tutorial in [True, False]:
                attributes = {'tutorial:complete': tutorial} if tutorial else {}
                for i in rng.sample(range(24), 3):
                    attributes['levelup:' + str(i)] = True
                p = player(levels={str(slot): level}, attributes=attributes, totalLevel=2376 if level == 99 and slot % 2 else 1200,
                           combatLevel=40 + slot, artificial=(slot == 7), emotes=list(range(22)) + ([37] if slot % 3 == 0 else []))
                sweep = 0 if (tutorial and level == 99 and slot in (5, 6)) else None
                yield case('levelup', [dict(op='levelup', slot=slot, amount=1 + slot % 3)], p), sweep
    # Flashing icons only.
    for slot in [-2, -1, 0, 5, 23, 24]:
        attributes = {'levelup:' + str(i): (i % 3 == 0) for i in range(25)}
        attributes['levelup:7'] = None
        yield case('flashing', [dict(op='flashing', slot=slot)], player(attributes={k: v for k, v in attributes.items() if v is not None})), \
            (0 if slot == -1 else None)
    yield case('flashing', [dict(op='flashing', slot=-1)], player()), None
    yield case('flashing', [dict(op='flashing', slot=25)], player()), None


# ------------------------------------------------------------------ EmoteManager and Emotes

EMOTE_ITEMS = {
    'legs': [None, 10396, 10394, 11022, 1075], 'hat': [None, 10392, 10398, 11021, 1153], 'chest': [None, 11020, 1115],
    'feet': [None, 11019, 4119], 'cape': [None, 9747, 9748, 9813, 12170, 9949, 1052],
}


def emote_cases():
    rng = random.Random(45)
    for k in range(60):
        unlocked = sorted(set(range(22)) | set(rng.sample(range(22, 45), rng.randrange(0, 23))))
        if k % 4 == 0 and 37 in unlocked:
            unlocked.remove(37)
        rng.shuffle(unlocked)
        yield case('emote_refresh', [dict(op='emote_refresh'), dict(op='emote_save_required')], player(emotes=unlocked)), (0 if k < 3 else None)
    for name in EMOTES:
        p = player(emotes=list(range(22)) + [40, 22])
        ops = [dict(op='emote_is_unlocked', emote=name), dict(op='emote_unlock', emote=name), dict(op='emote_unlock', emote=name),
               dict(op='emote_lock', emote=name), dict(op='emote_lock', emote=name), dict(op='emote_save_required')]
        yield case('emote_unlock', ops, p), (1 if name in ('IDEA', 'SKILLCAPE') else None)
    yield case('emote_lock', [dict(op='emote_lock', emote=None)], player()), None
    yield case('emote_lock', [dict(op='emote_is_unlocked', emote=None)], player()), None
    yield case('emote_for_id', [dict(op='emote_for_id', button=b) for b in range(-1, 50)], player()), None
    # Plays with each relevant piece of equipment.
    slots = {'hat': 0, 'cape': 1, 'chest': 4, 'legs': 7, 'feet': 10}
    for name in EMOTES:
        if name in CONTENT_EMOTES:
            continue
        variants = [{}]
        for part, ids in EMOTE_ITEMS.items():
            for item in ids[1:]:
                variants.append({str(slots[part]): item})
        variants.append({'0': 11021, '4': 11020, '7': 11022, '10': 11019})
        variants.append({'0': 11021, '4': 11020, '7': 11022})
        for worn in variants:
            durations = {'855': 3, '5312': 4, '4959': 9, '8525': 12}
            yield case('emote_play', [dict(op='emote_play', emote=name)], player(worn=worn, durations=durations)), \
                (0 if (name in ('BOW', 'SKILLCAPE', 'AIR_GUITAR', 'BLOW_KISS') and worn == variants[-2]) else None)
    # The button handler.
    for button in range(0, 50):
        if button in CONTENT_BUTTONS:
            yield case('emote_handle', [dict(op='emote_handle', button=button)], player(emoteLocked=True)), None
            continue
        for situation in range(6):
            p = player(emoteLocked=situation == 1, attacking=situation == 2, inCombat=situation == 3,
                       rights='ADMINISTRATOR' if situation == 4 else 'REGULAR_PLAYER', diaryComplete=situation == 5,
                       emotes=list(range(22)) + ([28, 29, 31] if button % 2 else []),
                       attributes={'emote-30': True, 'emote-31': True} if button % 3 == 0 else {})
            yield case('emote_handle', [dict(op='emote_handle', button=button), dict(op='emote_handle', button=(button + 1) % 34 + 30)
                                        if (button + 1) % 34 + 30 not in CONTENT_BUTTONS else dict(op='emote_save_required')], p), \
                (0 if situation == 0 and button in (2, 30, 33, 39) else None)
    # The Varrock diary task over the four Stronghold emotes.
    for order in [[30, 31, 32, 33], [33, 32, 31, 30], [30, 30, 31, 33, 32]]:
        p = player(emotes=list(range(22)) + [28, 29, 30, 31])
        yield case('emote_diary', [dict(op='emote_handle', button=b) for b in order], p), None


# ------------------------------------------------------------------ MusicPlayer and MusicZone

def music_cases():
    rng = random.Random(76)
    base = dict(songs=SONGS)
    for unlocked in [[], [62], [76, 62], [2, 3, 4, 5, 90, 91, 92, 82]]:
        for current, playing in [(0, False), (76, False), (2, True), (0, True)]:
            for looping in [False, True]:
                for guitar in [False, True]:
                    emotes = list(range(22)) + ([39] if guitar else [])
                    p = player(music=dict(unlocked=unlocked, current=current, playing=playing, looping=looping), emotes=emotes,
                               debug=(current == 2))
                    sweep = 0 if (unlocked == [] and current == 0 and not looping and not guitar) else None
                    yield dict(case('music_init', [dict(op='music_init'), dict(op='music_refresh')], p), **base), sweep
    for looping in [False, True]:
        for current in [0, 76, 2]:
            ops = []
            for sid in [76, 62, 300, 82, 83, 91, 999, 2, 2, 5]:
                ops.append(dict(op='music_unlock', id=sid, play=[None, True, False][sid % 3]))
                ops.append(dict(op='music_has_unlocked', id=sid))
            ops += [dict(op='music_has_air_guitar'), dict(op='music_play', id=4), dict(op='music_play', id=999), dict(op='music_replay'),
                    dict(op='music_play_default'), dict(op='music_toggle_looping'), dict(op='music_toggle_looping'),
                    dict(op='music_set_looping', looping=True), dict(op='music_set_looping', looping=False)]
            p = player(music=dict(unlocked=[], current=current, playing=current > 0, looping=looping), debug=looping)
            yield dict(case('music_unlock', ops, p), **base), (2 if not looping and current == 0 else None)
    # The air guitar emote at all songs and at 200.
    small = [[i, 'Song %d' % i, i] for i in range(1, 6)]
    yield dict(case('music_guitar', [dict(op='music_unlock', id=i, play=False) for i in range(1, 6)] + [dict(op='music_init')],
                    player(music=dict(unlocked=[]))), songs=small), 4
    many = [[i, 'Track %d' % i, i - 1] for i in range(1, 206)]
    yield dict(case('music_guitar', [dict(op='music_unlock', id=i, play=False) for i in range(195, 206)] + [dict(op='music_has_air_guitar')],
                    player(music=dict(unlocked=list(range(1, 195))))), songs=many), None
    yield dict(case('music_guitar', [dict(op='music_init')], player(music=dict(unlocked=list(range(1, 205))), emotes=list(range(22)) + [39])),
               songs=many), None
    # The 20-tick refresh, and its swallowed failure.
    for ticks in [0, 1, 19, 20, 40, 41]:
        for playing in [False, True]:
            p = player(ticks=ticks, music=dict(current=62, playing=playing))
            yield dict(case('music_tick', [dict(op='music_tick')], p), **base), (0 if ticks == 20 and not playing else None)
    # Music zones.
    for entity in ['player', 'npc']:
        for sid in [62, 999, 3]:
            yield dict(case('music_zone', [dict(op='music_zone_enter', id=sid, entity=entity)], player()), **base), None
    # Random sequences.
    for k in range(40):
        ops = []
        for _ in range(10):
            kind = rng.randrange(6)
            sid = rng.choice([s[0] for s in SONGS] + [999])
            if kind == 0:
                ops.append(dict(op='music_unlock', id=sid, play=rng.choice([None, True, False])))
            elif kind == 1:
                ops.append(dict(op='music_play', id=rng.choice([s[0] for s in SONGS])))
            elif kind == 2:
                ops.append(dict(op='music_toggle_looping'))
            elif kind == 3:
                ops.append(dict(op='music_replay'))
            elif kind == 4:
                ops.append(dict(op='music_init'))
            else:
                ops.append(dict(op='music_zone_enter', id=sid, entity='player'))
        p = player(music=dict(unlocked=rng.sample([s[0] for s in SONGS], 3)), debug=rng.random() < 0.3)
        yield dict(case('music_random', ops, p), **base), None


# ------------------------------------------------------------------ QuestRepository and Quest

def quest_row(name, index, button, points, configs, custom=False, custom_config=None, late=False):
    ordinal = [q[0] for q in QUESTS].index(name)
    return [name, index, button, points, configs, custom, custom_config, ordinal, QUESTS[ordinal][1], late]


def quest_cases():
    rng = random.Random(101)
    # Registration order differs from the enum order the TreeMap iterates.
    registry = [
        quest_row('DRAGON_SLAYER', 18, 17, 2, [176, 0, 1, 10]),
        quest_row('COOKS_ASSISTANT', 6, 5, 1, [29, 0, 1, 2]),
        quest_row('ANIMAL_MAGNETISM', 20, 19, 1, [0, 3185, 0, 1, 240], True, [0, 3185, 7]),
        quest_row('BLACK_KNIGHTS_FORTRESS', 1, 0, 3, [130, 0, 1, 4]),
        quest_row('ERNEST_THE_CHICKEN', 8, 7, 4, [32, 0, 1, 3, 9]),
        quest_row('GOBLIN_DIPLOMACY', 9, 8, 5, [62, 0, 1, 6], True, [62, 4]),
        quest_row('BIOHAZARD', 22, 21, 3, [68, 0, 1, 16]),
    ]
    for k in range(30):
        stages = {}
        for row in registry:
            stages[str(row[1])] = rng.choice([0, 0, 1, 10, 50, 99, 100, 101, 2 ** 31 - 1, -5])
        ops = [dict(op='quest_sync'), dict(op='quest_sync_points'), dict(op='quest_sync'), dict(op='quest_available'),
               dict(op='quest_complete_all')]
        sweep = 0 if k < 4 else None
        yield dict(case('quest_sync', ops, player(questStages=stages, questPoints=rng.randrange(0, 30))), quests=registry), sweep
    # A quest registered after the repository was built (no stage), a short configuration, custom configurations of every length.
    late = registry + [quest_row('DEMON_SLAYER', 3, 2, 3, [222, 0, 1, 3], late=True)]
    yield dict(case('quest_sync', [dict(op='quest_sync'), dict(op='quest_sync_points'), dict(op='quest_is_complete', quest='DEMON_SLAYER'),
                                   dict(op='quest_has_started', quest='DEMON_SLAYER'), dict(op='quest_stage', quest='DEMON_SLAYER')], player()),
               quests=late), None
    bad = registry + [quest_row('IMP_CATCHER', 11, 10, 1, [160, 0, 1])]
    yield dict(case('quest_sync', [dict(op='quest_sync')], player()), quests=bad), None
    for config in [[], [5], [5, 6], [5, 6, 7], [5, 6, 7, 8]]:
        custom = registry + [quest_row('PIRATES_TREASURE', 12, 11, 2, [71, 0, 1, 4], True, config)]
        yield dict(case('quest_sync', [dict(op='quest_sync')], player()), quests=custom), None
    # getConfig directly.
    for row in registry + [quest_row('IMP_CATCHER', 11, 10, 1, [160, 0, 1])]:
        for stage in [0, 1, 99, 100, 120, -1]:
            yield dict(case('quest_config', [dict(op='quest_config', quest=row[0], stage=stage)], player()),
                       quests=registry + ([row] if row not in registry else [])), None
    # The rest of the repository.
    for k in range(20):
        ops = []
        for _ in range(12):
            kind = rng.randrange(10)
            quest = rng.choice([r[0] for r in registry] + ['IMP_CATCHER', 'DEMON_SLAYER'])
            if kind == 0:
                ops.append(dict(op='quest_set_stage', quest=rng.choice([r[0] for r in registry]), stage=rng.choice([0, 5, 10, 100, -1])))
            elif kind == 1:
                ops.append(dict(op='quest_set_stage_nonmonotonic', quest=rng.choice([r[0] for r in registry]), stage=rng.choice([0, 5, 100])))
            elif kind == 2:
                ops.append(dict(op='quest_increment', value=rng.choice([1, 5, 2 ** 31 - 1])))
            elif kind == 3:
                ops.append(dict(op='quest_dock', value=rng.choice([1, 3, -2 ** 31])))
            elif kind == 4:
                ops.append(dict(op='quest_is_complete', quest=quest))
            elif kind == 5:
                ops.append(dict(op='quest_has_started', quest=quest))
            elif kind == 6:
                ops.append(dict(op='quest_stage', quest=quest))
            elif kind == 7:
                ops.append(dict(op='quest_for_button', button=rng.randrange(-1, 25)))
            elif kind == 8:
                ops.append(dict(op='quest_for_index', index=rng.randrange(-1, 25)))
            else:
                ops.append(dict(op='quest_sync'))
        yield dict(case('quest_repository', ops, player()), quests=registry), None


def cases():
    for generator in [weapon_cases, level_cases, emote_cases, music_cases, quest_cases]:
        yield from generator()
