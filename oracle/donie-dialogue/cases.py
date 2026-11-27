"""Cases for donie-dialogue: what the original does, recorded by oracle/donie-dialogue/generate.py.

`cases()` yields (case, sweep). A case is the dialogue-dsl world (npcs, attrs, items, male, ...: see
oracle/dialogue-dsl/Harness.java `world`) plus the kit's (`settings`: null for no settings, `serverName`)
and its `actions`. Actions: `register` (class: the scanned name), `openId` (key, npc: an index into npcs),
`handle` (c: interface, b: button, the Continue packet), `close`, `input` (v), `closeEvent` and the other
dialogue-dsl actions. `sweep` is the index of an action whose recorded calls each fail once in turn, or None.
"""

CLASS = 'content.region.misthalin.lumbridge.dialogue.DonieDialogue'
NPC = 2238

# Recorded call names the corpus must reach (the generator fails without them). Donie's own
# `GameWorld.settings?.name` is "GameWorld.settings" (the kit's stand-in records a read from a content class so); the
# dialogue core's @servername substitution reads the settings as "settings" on every dialogue it sends.
NEEDED_EVENTS = ['GameWorld.settings']

REGISTER = {'op': 'register', 'class': CLASS}
TALK = {'op': 'openId', 'key': NPC, 'npc': 0}


def H(b, c=None):
    """The Continue packet: interp.handle(c, b); options are buttons 2.. on their interface."""
    action = {'op': 'handle', 'b': b}
    if c is not None:
        action['c'] = c
    return action


def case(name, actions, sweep=None, **world):
    out = dict(name=name, actions=actions)
    out.update(world)
    out.setdefault('npcs', [NPC])
    return out, sweep


def cases():
    start = [REGISTER, TALK]
    # The greeting, then the three options. "Where am I?" ends after its answer (the next stage is the "howareyou"
    # label, not a hit stage); "How are you today?" runs to the end of its lines; the shoelace option opens
    # DonieDialogueShoelaceFile. The extra Continue after each end shows what a stale packet does.
    yield case('talk greeting', start, 1)
    yield case('talk options', start + [H(1)], 2)
    yield case('where am i', start + [H(1), H(2)] + [H(1)] * 3, 3)
    yield case('how are you', start + [H(1), H(3)] + [H(1)] * 8, 4)
    yield case('shoelace', start + [H(1), H(4)] + [H(1)] * 6, 3)
    yield case('shoelace answer', start + [H(1), H(4), H(1)], 4)
    # GameWorld.settings?.name: another name, and no settings. Without settings Donie's own read answers null (her
    # `?: "2009Scape"` default is computed on that pass) but the greeting already fails in the core's @servername
    # substitution (DialogueInterpreter.doSubstitutions), so the default never reaches a line: no sweep, nothing more
    # to see past the greeting.
    yield case('how are you named world', start + [H(1), H(3), H(1)], None, serverName='Gielinor')
    yield case('no settings', start, None, settings=None)
    yield case('female player', start + [H(1), H(4), H(1)], None, male=False)
    # Lifecycle around the plugin: not registered yet, closed mid-way, opened without an NPC, another NPC id.
    yield case('not registered', [TALK], 0)
    yield case('closed then continue', start + [{'op': 'close'}, H(1)], None)
    yield case('options closed', start + [H(1), {'op': 'close'}, H(2)], None)
    yield case('opened without npc', [REGISTER, {'op': 'openId', 'key': NPC}, H(1)], 1)
    yield case('button out of range', start + [H(1), H(9), H(1)], 3)
    yield case('registered only', [REGISTER], 0)
