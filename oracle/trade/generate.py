"""Run the unchanged request and trade classes on JDK 11 and record what they do.

Compiled unchanged: RequestManager.kt, RequestModule.kt, RequestType.kt,
TradeModule.kt, TradeContainer.java, TradeCloseEvent.java,
TradeInterfacePlugin.java, RunScript.kt, InputType.kt, Globals.kt,
IronmanManager.java, IronmanMode.java, MapDistance.java, Item.java,
Definition.java, Container.java with its event/listener/type/sort classes,
Component, ComponentDefinition, ComponentPlugin, CloseEvent, InterfaceType,
Plugin, Initializable, InterfaceManager.java, ClientInfo, IoBuffer and the
WindowsPane, Interface, CloseInterface, InterfaceConfig, Config, RunScriptPacket,
StringPacket, AccessMask and ContainerPacket writers with their contexts.
Everything else is an adapter under oracle/trade/adapters, with the original
text of the members named in SCOPE inserted at generation time. Launcher.java
runs each case in its own class loader. --check never updates a fixture, and
nothing here downloads: the pinned jars must already be under runs/.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/trade-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/trade'
CONFIG = ROOT / 'fixtures/interfaces/interface_configs.json'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')

ORIGINAL = [
    'core/game/node/entity/player/link/request/RequestManager.kt',
    'core/game/node/entity/player/link/request/RequestModule.kt',
    'core/game/node/entity/player/link/request/RequestType.kt',
    'core/game/node/entity/player/link/request/trade/TradeModule.kt',
    'core/game/node/entity/player/link/request/trade/TradeContainer.java',
    'core/game/node/entity/player/link/request/trade/TradeCloseEvent.java',
    'content/global/handlers/iface/TradeInterfacePlugin.java',
    'core/net/packet/in/RunScript.kt',
    'core/api/InputType.kt',
    'core/tools/Globals.kt',
    'core/game/node/entity/player/link/IronmanManager.java',
    'core/game/node/entity/player/link/IronmanMode.java',
    'core/game/world/map/MapDistance.java',
    'core/cache/def/Definition.java',
    'core/game/node/item/Item.java',
    'core/game/container/Container.java',
    'core/game/container/ContainerEvent.java',
    'core/game/container/ContainerListener.java',
    'core/game/container/ContainerType.java',
    'core/game/container/SortType.java',
    'core/game/component/Component.java',
    'core/game/component/ComponentDefinition.java',
    'core/game/component/ComponentPlugin.java',
    'core/game/component/CloseEvent.java',
    'core/game/component/InterfaceType.java',
    'core/plugin/Plugin.java',
    'core/plugin/Initializable.java',
    'core/game/node/entity/player/link/InterfaceManager.java',
    'core/game/node/entity/player/info/ClientInfo.java',
    'core/net/packet/IoBuffer.java',
    'core/net/packet/PacketHeader.java',
    'core/net/packet/Context.java',
    'core/net/packet/OutgoingPacket.java',
    'core/net/packet/out/WindowsPane.java',
    'core/net/packet/out/Interface.java',
    'core/net/packet/out/CloseInterface.java',
    'core/net/packet/out/InterfaceConfig.java',
    'core/net/packet/out/Config.java',
    'core/net/packet/out/RunScriptPacket.java',
    'core/net/packet/out/StringPacket.java',
    'core/net/packet/out/AccessMask.java',
    'core/net/packet/out/ContainerPacket.java',
    'core/net/packet/context/WindowsPaneContext.java',
    'core/net/packet/context/InterfaceContext.java',
    'core/net/packet/context/InterfaceConfigContext.java',
    'core/net/packet/context/ConfigContext.java',
    'core/net/packet/context/RunScriptContext.java',
    'core/net/packet/context/StringContext.java',
    'core/net/packet/context/AccessMaskContext.java',
    'core/net/packet/context/ContainerContext.java',
]
SCOPE = (
    'Unchanged RequestManager, RequestModule, RequestType, TradeModule, TradeContainer (with TradeListener), TradeCloseEvent, '
    'TradeInterfacePlugin, RunScript.processInput, InputType, Globals, IronmanManager/IronmanMode, MapDistance, Item, Definition, '
    'Container with ContainerEvent/ContainerListener/ContainerType/SortType, Component, ComponentDefinition, ComponentPlugin, CloseEvent, '
    'InterfaceType, Plugin, Initializable, the complete InterfaceManager, ClientInfo, IoBuffer and the WindowsPane, Interface, '
    'CloseInterface, InterfaceConfig, Config, RunScriptPacket, StringPacket, AccessMask and ContainerPacket writers with their contexts, '
    'on JDK 11, each case in a fresh class loader, with the pinned interface_configs.json. Original text inserted at generation time: '
    'Location.withinDistance and getDistance; PacketDispatch sendString, sendIfaceSettings, sendRunScript and sendInterfaceConfig; '
    'ContentAPI removeAttribute and both sendInputDialogue forms; Node and the ItemDefinition fields and methods Item, Container and '
    'the trade reach (isTradeable, hasAction, hasDestroyAction included); the ItemConfigParser constants; InterfaceConfigParser with a '
    'string reader. Recording adapters: Player (fields from the case, attributes and extensions as maps), PlayerDetails, the session '
    '(fixed mode, zero ISAAC), PacketDispatch.sendMessage, ContentAPI sendMessage/sendDialogue/setVarp (Config writer)/log, '
    'DialogueInterpreter, ZoneMonitor.canRequest, ItemDefinition.getExamine, InterfaceListeners, the open/close events, '
    'WeaponInterface, FriendsListInterface, spellbook 192, no familiar or autocast, GroundItemManager.create, PlayerMonitor.logTrade, '
    'AIRepository with DoublingMoney, AssistSession, GameWorld ticks/settings/Pulser and Pulse construction, and an inventory '
    'listener that records update slots and refreshes. Modelled in Support.java: the tail of processIfAction (component lookup, '
    'hidden check, getPlugin, handle), the InputPromptResponse branch around RunScript.processInput, running a submitted alert '
    'pulse body, and logout as InterfaceManager.close then closeSingleTab. Not exercised: pulse scheduling, the real '
    'InventoryListener, GameMessage bytes, examine text, AIPlayer bots, the duel arena and AssistSession content.'
)


def line(text, pattern):
    match = re.search(pattern, text, re.M)
    if not match:
        raise RuntimeError(f'{pattern} not found')
    return match.group()


def markers():
    def source(name):
        return c.read(SERVER / name)

    location = source('core/game/world/map/Location.kt')
    dispatch = source('core/game/node/entity/player/link/PacketDispatch.java')
    content = source('core/api/ContentAPI.kt')
    return {
        'ORIGINAL_WITHIN_DISTANCE': c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int'),
        'ORIGINAL_GET_DISTANCE': c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location): Double'),
        'ORIGINAL_SEND_STRING': c.method(dispatch, 'public void sendString(String string, int interfaceId, int lineId)'),
        'ORIGINAL_SEND_IFACE_SETTINGS': c.method(dispatch, 'public void sendIfaceSettings('),
        'ORIGINAL_SEND_RUN_SCRIPT': c.method(dispatch, 'public void sendRunScript(int id, String string, Object... objects)'),
        'ORIGINAL_SEND_INTERFACE_CONFIG': c.method(dispatch, 'public void sendInterfaceConfig(int interfaceId, int childId, boolean hide)'),
        'ORIGINAL_REMOVE_ATTRIBUTE': c.method(content, 'fun removeAttribute(entity: Entity, attribute: String)'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC': c.method(content, 'fun sendInputDialogue(player: Player, numeric: Boolean'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_TYPE': c.method(content, 'fun sendInputDialogue(player: Player, type: InputType'),
    }


def generated_adapters():
    """Adapters built from original text (the inventory and interfaces oracles' method)."""
    out = {}
    definition = c.read(SERVER / 'core/cache/def/impl/ItemDefinition.java')
    body = definition[definition.index('private static final Map<Integer, ItemDefinition> DEFINITIONS'):definition.index('public ItemDefinition()')]
    methods = ['public ItemDefinition()', 'public static ItemDefinition forId(', 'public static Map<Integer, ItemDefinition> getDefinitions()',
               'public boolean isStackable()', 'public int getValue()', 'public int getAlchemyValue(', 'public int getNoteId()',
               'public boolean isUnnoted()', 'public ItemPlugin getItemPlugin()', 'public static OptionHandler getOptionHandler(',
               'public boolean isTradeable()', 'public boolean hasAction(String optionName)', 'public boolean hasDestroyAction()']
    body += '\n'.join(c.method(definition, m) for m in methods)
    body += '\npublic String getExamine(){return oracle.Support.examine(this);}'
    out['core/cache/def/impl/ItemDefinition.java'] = (
        'package core.cache.def.impl;import java.util.*;import core.cache.def.Definition;import core.game.node.item.*;'
        'import core.game.interaction.OptionHandler;import core.game.system.config.ItemConfigParser;import core.tools.Log;'
        'import static core.api.ContentAPIKt.log;public class ItemDefinition extends Definition<Item>{' + body + '}')
    node = c.read(SERVER / 'core/game/node/Node.java')
    node_methods = ['public Node(', 'public String getName()', 'public int getIndex()', 'public void setIndex(', 'public DestinationFlag getDestinationFlag()']
    out['core/game/node/Node.java'] = (
        'package core.game.node;import core.game.interaction.*;import core.game.world.map.Location;public class Node {protected String name;'
        'protected int index;protected Location location;protected InteractPlugin interactPlugin;protected DestinationFlag destinationFlag;'
        + ''.join(c.method(node, m) for m in node_methods) + '}')
    config = c.read(SERVER / 'core/game/system/config/ItemConfigParser.kt')
    constants = ''.join('public static String ' + name + '=' + re.search(r'const val ' + name + r' = ("[^"]+")', config).group(1) + ';'
                        for name in ['HIGH_ALCHEMY', 'LOW_ALCHEMY', 'TRADEABLE'])
    out['core/game/system/config/ItemConfigParser.java'] = 'package core.game.system.config;public class ItemConfigParser {' + constants + '}'
    parser = c.read(SERVER / 'core/game/system/config/InterfaceConfigParser.kt')
    replaced = parser.replace('var reader: FileReader? = null', 'var reader: java.io.Reader? = null').replace(
        'FileReader(ServerConstants.CONFIG_PATH + "interface_configs.json")', 'java.io.StringReader(oracle.Support.config)')
    if replaced.count('oracle.Support.config') != 1 or 'java.io.Reader?' not in replaced:
        raise RuntimeError('InterfaceConfigParser text differs from the expected reader lines')
    out['core/game/system/config/InterfaceConfigParser.kt'] = replaced
    manager = c.read(SERVER / 'core/game/node/entity/player/link/InterfaceManager.java')
    names = set(re.findall(r'Components\.(\w+)', manager + c.read(SERVER / 'core/game/component/InterfaceType.java')))
    out['org/rs09/consts/Components.java'] = ('package org.rs09.consts;public class Components{'
                                             + ''.join(f'public static final int {name}={int(name.rsplit("_", 1)[1])};' for name in sorted(names)) + '}')
    return out


def pinned_json():
    pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))
    candidates = [RUN / 'json-simple-1.1.1.jar'] + sorted((ROOT / 'runs').glob('*-oracle/json-simple-1.1.1.jar'))
    for jar in candidates:
        if jar.is_file() and hashlib.sha256(jar.read_bytes()).hexdigest() == pin['sha256']:
            return jar
    raise RuntimeError('No json-simple-1.1.1.jar matching oracle/auth/json.lock.json under runs/; this generator never downloads')


# --------------------------------------------------------------------- corpus

FILLER = list(range(1100, 1124))
ITEMS = {
    995: dict(name='Coins', stackable=True, tradeable=True, examine='Lovely money!'),
    1511: dict(name='Logs', tradeable=True, noteId=1512, examine='A number of wooden logs.'),
    1512: dict(name='Logs', unnoted=False, tradeable=True, noteId=1511, examine='Swap this note at any bank for the equivalent item.'),
    4151: dict(name='Abyssal whip', tradeable=True, options=[None, 'Wield', None, None, 'Drop'], examine='A weapon from the abyss.'),
    6570: dict(name='Fire cape', tradeable=False, examine='A cape of fire.'),
    7462: dict(name='Barrows gloves', examine='The best in the Culinaromancer\'s collection.'),
    11174: dict(name='Shield left half', tradeable=False, examine='The left half of a shield.'),
    759: dict(name='Certificate', tradeable=False, examine='A certificate.'),
    9999: dict(name='Coins', stackable=True, tradeable=True, examine='Fake money.'),
    4045: dict(name='Explosive potion', tradeable=True, options=['Drink', None, None, None, 'Destroy'], examine='Explosive.'),
    11238: dict(name='Baby impling jar', tradeable=True, options=['Loot', None, None, None, 'Destroy'], examine='A jar.'),
    2: dict(name='Cannonball', stackable=True, tradeable=True, examine='Ammo for the Dwarf Cannon.'),
    1038: dict(name='Red partyhat', tradeable=True, examine='A nice hat from a cracker.'),
}
for number, item in enumerate(FILLER):
    ITEMS[item] = dict(name=f'Item {number}', tradeable=True, examine=f'Item {number}.')
INT_MAX = (1 << 31) - 1


def P(name, inv=(), x=3222, y=3222, z=0, **kw):
    spec = dict(name=name, inv=[list(r) for r in inv], x=x, y=y, z=z, ip=f'10.0.0.{len(name)}{name[0]}', mac=f'mac-{name}',
                host=f'pc-{name}')
    spec.update(kw)
    return spec


def a(op, **kw):
    return dict(op=op, **kw)


def req(p, t, kind='trade'):
    return a('request', p=p, t=t, kind=kind)


def btn(p, iface, opcode, button=0, slot=0, item=-1):
    return a('button', p=p, iface=iface, opcode=opcode, button=button, slot=slot, item=item)


OFFER = {1: 155, 5: 196, 10: 124, 'all': 199, 'x': 234}


def offer(p, slot, amount=1):
    return btn(p, 336, OFFER[amount], 0, slot)


def withdraw(p, slot, amount=1):
    return btn(p, 335, OFFER[amount], 30, slot)


def accept(p):
    return btn(p, 335, 155, 16)


def decline(p):
    return btn(p, 335, 155, 18)


def accept2(p):
    return btn(p, 334, 155, 20)


def decline2(p):
    return btn(p, 334, 155, 21)


def text(p, value):
    return a('input', p=p, text=value)


def number(p, value):
    return a('input', p=p, int=value)


OPEN = [req(0, 1), req(1, 0)]
BOTH = [accept(0), accept(1)]
BOTH2 = [accept2(0), accept2(1)]


def requests():
    cases = []

    def case(actions, players=None, **kw):
        row = dict(items={str(k): v for k, v in ITEMS.items()}, ticks=kw.pop('ticks', 100), players=players or [P('alice'), P('bob')],
                   actions=actions)
        row.update(kw)
        cases.append(row)

    rich = [(0, 995, 1000), (1, 1511, 1), (2, 1511, 1), (3, 1512, 50), (4, 4151, 1), (5, 6570, 1), (6, 2, 300)]
    other = [(0, 995, 5), (1, 1038, 1)]

    # Requests: checks, messages and their order.
    case([req(0, 0)])
    case([req(0, 1)])
    case([req(0, 1), req(0, 1)])
    case(OPEN)
    case([req(0, 1, 'assist'), req(1, 0, 'assist')])
    case([req(0, 1, 'assist'), req(1, 0)])
    case([req(0, 1), req(1, 0, 'assist')])
    case([req(0, 1), a('tick', n=100000), req(1, 0)])
    case([req(0, 1), req(0, 2), req(1, 0)], [P('alice'), P('bob'), P('carol')])
    case([req(0, 1), req(2, 1), req(1, 0), req(1, 2)], [P('alice'), P('bob'), P('carol')])
    case([req(1, 0), req(0, 1), req(2, 0)], [P('alice'), P('bob'), P('carol')])
    for dx, dy in [(15, 0), (16, 0), (0, -15), (0, 16), (9, 12), (10, 12), (11, 11), (-11, -10), (65536, 0), (46341, 0), (46340, 0),
                   (65536, 65536), (-65536, 15), (65537, 0), (1 << 30, 0)]:
        case([req(0, 1)], [P('alice'), P('bob', x=3222 + dx, y=3222 + dy)])
    case([req(0, 1)], [P('alice'), P('bob', z=1)])
    case([req(0, 1), a('move', p=1, x=3300, y=3222, z=0), req(1, 0)])
    case([req(0, 1)], [P('alice'), P('bob', active=False)])
    case([a('open', p=1, id=12), req(0, 1)])
    case([a('open', p=0, id=12), req(0, 1)])
    case([a('open', p=0, id=12), req(0, 1), req(1, 0)])
    for busy in [99, 100, 101, -5]:
        case([a('attr', p=1, key='busy', value=busy), req(0, 1)])
        case([a('attr', p=0, key='busy', value=busy), req(0, 1)])
    case([req(0, 1)], zoneRefuse=[0])
    case([req(0, 1), req(1, 0)], zoneRefuse=[1])
    case([req(0, 1, 'assist')], zoneRefuse=[0])
    case([req(0, 1)], [P('alice'), P('Bob', artificial=True)], bots={'bob': 'doubling'})
    case([req(0, 1)], [P('alice'), P('bob', artificial=True)], bots={'bob': 'other'})
    case([req(0, 1)], [P('alice'), P('bob', artificial=True)])
    case([req(0, 1)], [P('alice'), P('bob')], bots={'bob': 'doubling'})
    case([req(0, 1, 'assist')], [P('alice'), P('bob', artificial=True)], bots={'bob': 'doubling'})
    case([req(0, 1), req(1, 0), decline(0), req(0, 1), a('tick', n=1), req(0, 1), a('tick', n=1), req(0, 1), req(1, 0)])
    case([req(0, 1), req(1, 0), req(2, 0), req(2, 1), req(0, 2)], [P('alice'), P('bob'), P('carol')])
    case(OPEN + [a('tick', n=5), req(0, 2), req(2, 0)], [P('alice', rich), P('bob'), P('carol')])
    case(OPEN + [offer(0, 0, 'all'), a('tick', n=5), req(0, 2)], [P('alice', rich), P('bob'), P('carol')])
    case(OPEN + [offer(0, 0, 'all'), a('tick', n=5), req(0, 2), req(2, 0)], [P('alice', rich), P('bob'), P('carol')])
    case([req(0, 1), req(1, 0)], ticks=INT_MAX)
    case([req(0, 1), req(1, 0), a('tick', n=1), req(0, 1)], ticks=INT_MAX - 1)
    case([req(0, 1, 'assist'), req(1, 0, 'assist'), req(0, 1, 'assist')])
    case([req(0, 1)], failures={'message': 1})
    case(OPEN, failures={'dialogueClose': 2})
    case(OPEN, failures={'zone': 2})

    # Opening, simple exchanges of each kind.
    case(OPEN, [P('alice', rich), P('bob', other)])
    for slot in range(7):
        for amount in [1, 5, 10, 'all']:
            case(OPEN + [offer(0, slot, amount)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 0, 'all'), offer(0, 3, 'all'), offer(1, 0, 'all'), offer(1, 1)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 1), offer(0, 2), offer(0, 4), offer(1, 1)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(1, 1)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 0, 5), offer(0, 0, 5), offer(0, 0, 10)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 6, 'x'), text(0, '25')] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])

    # Untradeable, ironman, bot and name rules; dev mode.
    tricky = [(0, 6570, 1), (1, 7462, 1), (2, 11174, 1), (3, 759, 1), (4, 9999, 50), (5, 4045, 1), (6, 11238, 1), (7, 995, 10), (8, 1038, 1)]
    for dev in [False, True]:
        for slot in range(9):
            case(OPEN + [offer(0, slot)], [P('alice', tricky), P('bob')], dev=dev)
    for mode in ['STANDARD', 'HARDCORE', 'ULTIMATE']:
        for who in [0, 1]:
            players = [P('alice', tricky), P('bob')]
            players[who]['ironman'] = mode
            case(OPEN + [offer(0, 2), offer(0, 3), offer(0, 7), offer(0, 8)], players)
    case(OPEN + [offer(0, 7), a('ironman', p=1, mode='STANDARD'), offer(0, 8), a('ironman', p=1, mode='NONE'), offer(0, 8)],
         [P('alice', tricky), P('bob')])
    for field in ['ip', 'mac', 'host', None]:
        for bot in [0, 1, None]:
            players = [P('alice', tricky), P('bob')]
            if field:
                players[1][field] = players[0][field]
            if bot is not None:
                players[bot]['bot'] = True
            case(OPEN + [offer(0, 7), offer(0, 2)], players)
    case(OPEN + [offer(0, 7)], [P('alice', tricky, bot=True), P('bob', ip='10.0.0.5a')], dev=True)

    # Boundary amounts, input parsing and overflow.
    coins = [(0, 995, INT_MAX), (1, 1511, 1), (2, 1512, 7)]
    for value in ['0', '1', '2', '7', '1000', '2147483647', '2147483648', '-1', '-2147483648', '', 'abc', '1k', '1K', '10k', '1kk', '1m', '1M',
                  '2147483647k', ' 5', '5 ', '+5', '00012', '١٢', '1e3']:
        for slot in [0, 2]:
            case(OPEN + [offer(0, slot, 'x'), text(0, value)], [P('alice', coins), P('bob')])
    for value in [0, -1, 1, 5, INT_MAX]:
        case(OPEN + [offer(0, 0, 'x'), number(0, value)], [P('alice', coins), P('bob')])
    for value in ['1', '3', '1m', '1M', '1k', '2147483648', 'x', '']:
        case(OPEN + [offer(0, 0, 'all'), offer(0, 2, 'all'), withdraw(0, 0, 'x'), text(0, value)], [P('alice', coins), P('bob')])
        case(OPEN + [offer(0, 2, 'all'), withdraw(0, 0, 'x'), text(0, value)], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 0, 'x'), a('close', p=0), text(0, '5')], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 0, 'x'), decline(1), text(0, '5')], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 0, 'x'), offer(0, 2), text(0, '5')], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 0, 'x'), offer(0, 2, 'x'), text(0, '5'), text(0, '5')], [P('alice', coins), P('bob')])
    case([offer(0, 0, 'x'), text(0, '5')], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 0, 'all'), a('invAdd', p=0, id=995, amount=5), offer(0, 0, 'all')], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 0, 'all'), a('invAdd', p=0, id=995, amount=5), offer(0, 0, 'x'), text(0, '5')], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 0, 'all')] + BOTH + BOTH2, [P('alice', coins), P('bob', [(3, 995, 100)])])
    case(OPEN + [offer(0, 0, 'x'), text(0, '1000')] + BOTH + BOTH2, [P('alice', coins), P('bob', [(3, 995, INT_MAX - 400)])])
    case(OPEN + [offer(0, 0, 'x'), text(0, '400')] + BOTH + BOTH2, [P('alice', coins), P('bob', [(3, 995, INT_MAX - 400)])])
    case(OPEN + [offer(0, 0, 'x'), text(0, '401')] + BOTH + BOTH2, [P('alice', coins), P('bob', [(3, 995, INT_MAX - 400)])])
    case(OPEN + [offer(0, 0, 'all'), offer(1, 0, 'all')] + BOTH + BOTH2, [P('alice', coins), P('bob', [(0, 995, INT_MAX)])])
    for slot in [-1, 27, 28, 29, 1000, -2147483648]:
        case(OPEN + [offer(0, slot), withdraw(0, slot)], [P('alice', coins), P('bob')])
    case(OPEN + [offer(0, 1), withdraw(0, 0, 'all'), withdraw(0, 0), withdraw(0, 1)], [P('alice', coins), P('bob')])
    case(OPEN + [btn(0, 336, 64), btn(0, 335, 64), btn(0, 335, 155, 99), btn(0, 334, 155, 20), btn(0, 548, 155)], [P('alice', coins), P('bob')])

    # Stackables, notes and unstackables piling, splitting, removal order and shift.
    logs = [(slot, 1511, 1) for slot in range(10)] + [(10, 1512, 40), (11, 995, 77)]
    case(OPEN + [offer(0, 0, 5), offer(0, 6, 10), offer(0, 10, 5), withdraw(0, 0), withdraw(0, 0, 5), withdraw(0, 4, 'all')], [P('alice', logs), P('bob')])
    case(OPEN + [offer(0, 0, 'all'), withdraw(0, 0, 10), offer(0, 11), offer(0, 10, 'all'), withdraw(0, 1, 'x'), text(0, '3')], [P('alice', logs), P('bob')])
    case(OPEN + [offer(0, 3), offer(0, 1), offer(0, 2), withdraw(0, 0), offer(0, 0)] + BOTH + BOTH2, [P('alice', logs), P('bob')])
    case(OPEN + [offer(0, 0, 'all'), offer(0, 10, 'all'), offer(0, 11, 'all')] + BOTH + BOTH2, [P('alice', logs), P('bob', [(s, 1038, 1) for s in range(20)])])
    many = [(slot, item, 1 + slot) for slot, item in enumerate(FILLER)] + [(24, 995, 1234567), (25, 2, 1), (26, 1512, 2), (27, 4151, 1)]
    for count in [13, 14, 15, 16, 20, 28]:
        case(OPEN + [offer(0, s, 'all') for s in range(count)] + BOTH + BOTH2,
             [P('alice', many), P('bob', [(0, 995, 1000000), (1, 1038, 1)])])
    case(OPEN + [offer(0, s, 'all') for s in range(28)] + [offer(1, 0, 'all'), offer(1, 1)] + BOTH + BOTH2,
         [P('alice', many), P('bob', [(0, 995, 1000000), (1, 1038, 1)])])

    # Full inventories on either side; the check, the reset and the messages.
    full = [(s, 1038, 1) for s in range(28)]
    case(OPEN + [offer(0, 0)] + BOTH, [P('alice', rich), P('bob', full)])
    case(OPEN + [offer(1, 0)] + BOTH, [P('alice', full), P('bob', rich)])
    case(OPEN + [offer(0, 0)] + BOTH + [offer(1, 0)] + BOTH + BOTH2, [P('alice', rich), P('bob', full)])
    case(OPEN + [offer(0, 1)] + BOTH, [P('alice', rich), P('bob', full)])
    case(OPEN + [offer(0, 1), offer(1, 0)] + BOTH + BOTH2, [P('alice', rich), P('bob', full)])
    case(OPEN + [offer(0, 3, 'all')] + BOTH, [P('alice', rich), P('bob', full[:27] + [(27, 1512, 3)])])
    case(OPEN + [offer(0, 1), offer(0, 2), offer(1, 0)] + BOTH + BOTH2, [P('alice', rich), P('bob', full[:27])])
    case(OPEN + [offer(0, s, 'all') for s in range(28)] + BOTH + BOTH2, [P('alice', full), P('bob')])
    case(OPEN + [offer(0, s) for s in range(28)] + [offer(1, s) for s in range(28)] + BOTH + BOTH2, [P('alice', full), P('bob', full)])
    case(OPEN + [offer(0, 0), a('invAdd', p=0, id=1038, amount=1), withdraw(0, 0)], [P('alice', full), P('bob')])
    case(OPEN + [offer(0, 0), offer(0, 1), a('invAdd', p=0, id=1038, amount=2), withdraw(0, 0), withdraw(0, 0, 'all')], [P('alice', full), P('bob')])

    # The final exchange when a side can no longer take the items.
    for fill in [1, 2, 26, 27]:
        case(OPEN + [offer(0, 1), offer(0, 2), offer(0, 3, 'all'), offer(0, 0, 'all')] + BOTH
             + [a('invAdd', p=1, id=1038, amount=fill)] + BOTH2, [P('alice', rich), P('bob')])
    case(OPEN + [offer(0, 0, 'all')] + BOTH + [a('invAdd', p=1, id=995, amount=INT_MAX - 999)] + BOTH2, [P('alice', rich), P('bob')])
    case(OPEN + [offer(0, 0, 'all')] + BOTH + [a('invAdd', p=1, id=995, amount=INT_MAX)] + BOTH2, [P('alice', rich), P('bob')])
    case(OPEN + [offer(0, 1), offer(1, 1)] + BOTH + [a('invAdd', p=0, id=1038, amount=27), a('invAdd', p=1, id=1511, amount=27)] + BOTH2,
         [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 1)] + BOTH + [a('invReplace', p=0, slot=2, id=None), a('invAdd', p=1, id=1038, amount=28)] + BOTH2,
         [P('alice', rich), P('bob', other)])

    # Accepting, modifying after acceptance, the warning, the alert pulse.
    case(OPEN + [offer(0, 0, 'all'), accept(0), offer(1, 0)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 0, 'all'), accept(0), offer(0, 1)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 0, 'all'), offer(0, 1), accept(1), withdraw(0, 1), a('pulse', p=0), a('tick', n=3), offer(0, 2), a('pulse', p=0),
                 a('tick', n=5), offer(0, 3), a('pulse', p=0), a('pulse', p=1)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 0, 'all'), accept(0), accept(0), accept(1), accept(1)] + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 0, 'all')] + BOTH + [accept2(0), accept2(0), offer(0, 1), withdraw(0, 0), accept2(1)], [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 1), offer(0, 2), withdraw(0, 0), a('tick', n=7), withdraw(0, 0), a('tick', n=1), offer(0, 4), a('pulse', p=0)],
         [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 1), withdraw(0, 0), decline(0), a('pulse', p=0)], [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 1), offer(1, 1), withdraw(0, 0), withdraw(1, 0)] + BOTH + BOTH2, [P('alice', rich), P('bob', other)])
    case(OPEN + [offer(0, 1), withdraw(0, 0), a('tick', n=8), offer(0, 2), a('tick', n=1), offer(0, 4)], [P('alice', rich), P('bob', other)],
         ticks=INT_MAX - 3)

    # Examine on each screen.
    case(OPEN + [btn(0, 336, 9, 0, 0), btn(0, 336, 9, 0, 1), offer(0, 3), btn(0, 335, 9, 30, 0), btn(1, 335, 9, 32, 0), btn(1, 335, 9, 30, 0)],
         [P('alice', rich), P('bob', other)])
    case(OPEN + [btn(0, 336, 9, 0, 20)], [P('alice', rich), P('bob', other)])
    case(OPEN + [btn(0, 335, 9, 32, 0)], [P('alice', rich), P('bob', other)])

    # Declining, closing and logging out at every stage.
    stages = {
        'empty': [],
        'offered': [offer(0, 0, 'all'), offer(0, 1), offer(1, 0)],
        'accepted': [offer(0, 0, 'all'), offer(1, 0), accept(0)],
        'both': [offer(0, 0, 'all'), offer(1, 0)] + BOTH,
        'second': [offer(0, 0, 'all'), offer(1, 0)] + BOTH + [accept2(1)],
        'done': [offer(0, 0, 'all'), offer(1, 0)] + BOTH + BOTH2,
    }
    for name, before in stages.items():
        for who in [0, 1]:
            for end in [[decline(who)], [decline2(who)], [a('close', p=who)], [a('logout', p=who)], [btn(who, 336, 155, 0, 0)],
                        [a('close', p=who), a('close', p=1 - who)], [req(who, 2)], [a('open', p=who, id=12)]]:
                case(OPEN + before + end + [req(0, 1), req(1, 0)], [P('alice', rich), P('bob', other), P('carol')])
    case(OPEN + [a('logout', p=0), a('logout', p=1)])
    case(OPEN + [offer(0, 0, 'all'), a('logout', p=0), req(1, 0)], [P('alice', rich), P('bob', other)])

    # Injected failures: atomicity of the exchange and of the close.
    for event, at in [('logTrade', 1), ('ground', 1), ('message', 3), ('invUpdate', 3), ('botItemsReceived', 1), ('send', 200),
                      ('dialogueClose', 3), ('varp', 1), ('listenerClose', 2), ('invRefresh', 2)]:
        players = [P('alice', rich), P('bob', other)]
        steps = OPEN + [offer(0, 0, 'all'), offer(0, 1), offer(1, 0)] + BOTH
        if event == 'ground':
            steps = steps + [a('invAdd', p=1, id=1038, amount=27)]
        case(steps + BOTH2 + [decline(0), req(0, 1), req(1, 0)], players, failures={event: at}, bots={'alice': 'doubling'})
    case(OPEN + [offer(0, 0, 'all')] + BOTH + BOTH2, [P('alice', rich), P('bob', other)], bots={'alice': 'other'})
    case(OPEN + [offer(0, 0, 'all')] + BOTH + BOTH2, [P('alice', rich), P('Bob', other)], bots={'bob': 'doubling', 'alice': 'doubling'})
    return cases


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)

    lock_names = json.loads(c.read(ROOT / 'oracle/login/kotlin.lock.json'))
    for name, entry in lock_names.items():
        jar = ROOT / 'runs/kotlin-1.8.20' / name
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != entry['sha256']:
            raise RuntimeError(f'{jar} is missing or differs from its pin; this generator never downloads')
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libraries = [stdlib, pinned_json()]

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    texts = generated_adapters()
    for path in sorted((HERE / 'adapters').rglob('*')):
        if path.is_file():
            texts[path.relative_to(HERE / 'adapters').as_posix()] = c.read(path)
    for name, text in sorted(texts.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    for name in ['Support.java', 'Launcher.java']:
        c.read(HERE / name)
    java.append(HERE / 'Support.java')
    config = CONFIG.read_bytes()
    c.read(CONFIG)

    classes, launcher = RUN / 'classes', RUN / 'launcher'
    for path in [classes, launcher]:
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-d', launcher, HERE / 'Launcher.java'])

    cases = requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', f'-Dtrade.config={CONFIG}',
               '-cp', launcher, 'oracle.Launcher', classes, *libraries]
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=3600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-500:])
    outputs = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(outputs) != len(cases):
        raise RuntimeError(f'{len(outputs)} results for {len(cases)} cases')
    rows = [dict(case, expected=output['expected']) for case, output in zip(cases, outputs)]
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    events, errors, operations = {}, {}, {}
    for row in rows:
        for action, step in zip([dict(op='setup')] + row['actions'], row['expected']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if step['error'] is not None:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    exchanges = sum(1 for row in rows for step in row['expected'] for event in step['events'] if event[0] == 'logTrade')
    coverage = dict(cases=len(rows), actions=sum(operations.values()), operations=dict(sorted(operations.items())),
                    recordedCalls=sum(events.values()), events=dict(sorted(events.items())), errors=dict(sorted(errors.items())),
                    exchanges=exchanges, injectedFailures=sum(len(row.get('failures', {})) for row in rows))
    for needed in ['message', 'send', 'varp', 'logTrade', 'ground', 'alertSubmit', 'sendInput', 'dialogue', 'examine', 'botTradeReceived',
                   'botItemsReceived', 'assistOpen', 'zone', 'contentMessage', 'invUpdate', 'invRefresh', 'dialogueClose']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        interfaceConfigSha256=hashlib.sha256(config.replace(b'\r', b'')).hexdigest(), coverage=coverage,
        sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original request and trade cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{exchanges} completed exchanges)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Trade fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary} under {FIXTURES}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
