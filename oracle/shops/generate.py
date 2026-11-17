"""Run the unchanged shop classes on JDK 11 and record what they do.

Compiled unchanged: Shop.kt, Shops.kt, InterfaceListener.kt,
InterfaceListeners.kt, IntType.kt, ContentInterface.kt, StartupListener.kt,
TickListener.kt, IfaceSettingsBuilder.kt, InputType.kt, RunScript.kt,
TickUtils.kt, DialogueConst.kt, IronmanManager.java, IronmanMode.java,
Definition.java, Item.java, Container.java with its event/listener/type/sort
classes, Component, ComponentDefinition, ComponentPlugin, CloseEvent,
InterfaceType, Plugin, Initializable, InterfaceManager.java, ClientInfo, IoBuffer
and the WindowsPane, Interface, CloseInterface, InterfaceConfig, Config,
RunScriptPacket, StringPacket, AccessMask and ContainerPacket writers with their
contexts. Everything else is an adapter under oracle/shops/adapters, with the
original text of the members named in SCOPE inserted at generation time.
Launcher.java runs each case in its own class loader. --check never updates a
fixture, and nothing here downloads: the pinned jars must already be under runs/.
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
RUN = ROOT / 'runs/shops-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/shops'
CONFIG = ROOT / 'fixtures/interfaces/interface_configs.json'
PINNED = ROOT / 'fixtures/startup-loaders/shops.json'
CASES_DIR = 'runs/shops-oracle/cases'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')

ORIGINAL = [
    'core/game/shops/Shop.kt',
    'core/game/shops/Shops.kt',
    'core/game/interaction/InterfaceListener.kt',
    'core/game/interaction/InterfaceListeners.kt',
    'core/game/interaction/IntType.kt',
    'core/api/ContentInterface.kt',
    'core/api/StartupListener.kt',
    'core/api/TickListener.kt',
    'core/api/IfaceSettingsBuilder.kt',
    'core/api/InputType.kt',
    'core/net/packet/in/RunScript.kt',
    'core/tools/TickUtils.kt',
    'core/tools/DialogueConst.kt',
    'core/game/node/entity/player/link/IronmanManager.java',
    'core/game/node/entity/player/link/IronmanMode.java',
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
    'Unchanged Shop.kt (Shop, ShopItem, ShopListener), Shops.kt (start-up listener, tick listener, NPC trade listeners, '
    'destination override, interface listeners for 620/621, command definitions), InterfaceListener/InterfaceListeners, IntType, '
    'ContentInterface, StartupListener, TickListener, IfaceSettingsBuilder, InputType, RunScript.processInput, TickUtils, DialogueConst, '
    'IronmanManager/IronmanMode, Definition, Item, Container with ContainerEvent/ContainerListener/ContainerType/SortType, Component, '
    'ComponentDefinition, ComponentPlugin, CloseEvent, InterfaceType, Plugin, Initializable, the complete InterfaceManager, ClientInfo, '
    'IoBuffer and the WindowsPane, Interface, CloseInterface, InterfaceConfig, Config, RunScriptPacket, StringPacket, AccessMask and '
    'ContainerPacket writers with their contexts, on JDK 11, each case in a fresh class loader, with the pinned interface_configs.json. '
    'Original text inserted at generation time: ContentAPI getAttribute, setAttribute, removeAttribute, openInterface, setInterfaceText, '
    'inInventory, hasSpaceFor, itemDefinition, getItemName, getWorldTicks and both sendInputDialogue forms; PacketDispatch sendString, '
    'sendIfaceSettings, sendRunScript and sendInterfaceConfig; Location.transform(diffX, diffY, z); Node and the ItemDefinition fields '
    'and methods the shop reaches (getValue, getAlchemyValue, getNoteId, isUnnoted, isTradeable, hasAction, hasDestroyAction, both '
    'hasShopCurrencyValue); the ItemConfigParser constants; InterfaceConfigParser with a string reader; Events.kt\'s Event and the two '
    'shop events. ContentAPI removeItem/addItem keep their INVENTORY branch. Recording adapters: Player (attributes as a map, uid from '
    'the case), PlayerDetails, the session (fixed mode, zero ISAAC), Player.sendMessage/debug/dispatch, ContentAPI sendMessage/'
    'sendDialogue/sendNPCDialogue/setVarp/log and the quest reads, getServerConfig().getBoolean, DialogueInterpreter, '
    'ItemDefinition.getExamine, GrandExchange.addBotOffer, TanningProduct.open, NPC (id, facing_booth, direction, location), Direction '
    'steps, InteractionListener/Commands registration, WeaponInterface, FriendsListInterface, spellbook 192, no familiar or autocast, '
    'ServerConstants (CONFIG_PATH, SERVER_NAME, PLAYER_STOCK_CLEAR_INTERVAL, PLAYER_STOCK_RECIRCULATE from the case), and an '
    'inventory listener that records update slots and refreshes. Modelled in Support.java: the tail of processIfAction (component '
    'lookup, hidden check, ButtonClickEvent, InterfaceListeners.run, plugin), the InputPromptResponse branch around RunScript.'
    'processInput, the NPC listener choice (id-specific before option-only), the worker\'s per-tick order (tick listener, then '
    'GameWorld.pulse, skipped when the listener throws), and logout as InterfaceManager.close then closeSingleTab. Not exercised: '
    'InteractionListeners dispatch and walking to the NPC, the command bodies, the real InventoryListener, GameMessage bytes and '
    'the real item cache (definitions are authored inputs, including those for the pinned rows).'
)


def markers():
    def source(name):
        return c.read(SERVER / name)

    location = source('core/game/world/map/Location.kt')
    dispatch = source('core/game/node/entity/player/link/PacketDispatch.java')
    content = source('core/api/ContentAPI.kt')
    return {
        'ORIGINAL_TRANSFORM': re.search(r'^\s*fun transform\(diffX: Int, diffY: Int, z: Int\): Location = .*$', location, re.M).group().strip(),
        'ORIGINAL_SEND_STRING': c.method(dispatch, 'public void sendString(String string, int interfaceId, int lineId)'),
        'ORIGINAL_SEND_IFACE_SETTINGS': c.method(dispatch, 'public void sendIfaceSettings('),
        'ORIGINAL_SEND_RUN_SCRIPT': c.method(dispatch, 'public void sendRunScript(int id, String string, Object... objects)'),
        'ORIGINAL_SEND_INTERFACE_CONFIG': c.method(dispatch, 'public void sendInterfaceConfig(int interfaceId, int childId, boolean hide)'),
        'ORIGINAL_GET_ATTRIBUTE': c.method(content, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T'),
        'ORIGINAL_SET_ATTRIBUTE': c.method(content, 'fun <T> setAttribute(entity: Entity, attribute: String, value: T)'),
        'ORIGINAL_REMOVE_ATTRIBUTE': c.method(content, 'fun removeAttribute(entity: Entity, attribute: String)'),
        'ORIGINAL_OPEN_INTERFACE': c.method(content, 'fun openInterface(player: Player, id: Int)'),
        'ORIGINAL_SET_INTERFACE_TEXT': c.method(content, 'fun setInterfaceText(player: Player, string: String, iface: Int, child: Int)'),
        'ORIGINAL_IN_INVENTORY': c.method(content, 'fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean'),
        'ORIGINAL_HAS_SPACE_FOR': c.method(content, 'fun hasSpaceFor(player: Player, item: Item): Boolean'),
        'ORIGINAL_ITEM_DEFINITION': c.method(content, 'fun itemDefinition(id: Int): ItemDefinition'),
        'ORIGINAL_GET_ITEM_NAME': c.method(content, 'fun getItemName(id: Int): String'),
        'ORIGINAL_GET_WORLD_TICKS': c.method(content, 'fun getWorldTicks(): Int'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_NUMERIC': c.method(content, 'fun sendInputDialogue(player: Player, numeric: Boolean'),
        'ORIGINAL_SEND_INPUT_DIALOGUE_TYPE': c.method(content, 'fun sendInputDialogue(player: Player, type: InputType'),
    }


def generated_adapters():
    """Adapters built from original text (the inventory, interfaces and trade oracles' method)."""
    out = {}
    definition = c.read(SERVER / 'core/cache/def/impl/ItemDefinition.java')
    body = definition[definition.index('private static final Map<Integer, ItemDefinition> DEFINITIONS'):definition.index('public ItemDefinition()')]
    methods = ['public ItemDefinition()', 'public static ItemDefinition forId(', 'public static Map<Integer, ItemDefinition> getDefinitions()',
               'public boolean isStackable()', 'public int getValue()', 'public int getAlchemyValue(', 'public int getNoteId()',
               'public boolean isUnnoted()', 'public ItemPlugin getItemPlugin()', 'public static OptionHandler getOptionHandler(',
               'public boolean isTradeable()', 'public boolean hasAction(String optionName)', 'public boolean hasDestroyAction()',
               'public boolean hasShopCurrencyValue(String currency)', 'public boolean hasShopCurrencyValue(int currency)']
    body += '\n'.join(c.method(definition, m) for m in methods)
    body += '\npublic String getExamine(){return oracle.Support.examine(this);}'
    out['core/cache/def/impl/ItemDefinition.java'] = (
        'package core.cache.def.impl;import java.util.*;import core.cache.def.Definition;import core.game.node.item.*;'
        'import core.game.interaction.OptionHandler;import core.game.system.config.ItemConfigParser;import core.tools.Log;'
        'import org.rs09.consts.Items;import static core.api.ContentAPIKt.log;public class ItemDefinition extends Definition<Item>{' + body + '}')
    node = c.read(SERVER / 'core/game/node/Node.java')
    node_methods = ['public Node(', 'public String getName()', 'public int getIndex()', 'public void setIndex(', 'public DestinationFlag getDestinationFlag()',
                    'public Location getLocation()', 'public Direction getDirection()']
    out['core/game/node/Node.java'] = (
        'package core.game.node;import core.game.interaction.*;import core.game.world.map.Location;import core.game.world.map.Direction;'
        'public class Node {protected String name;protected int index;protected Location location;protected InteractPlugin interactPlugin;'
        'protected DestinationFlag destinationFlag;protected Direction direction;'
        + ''.join(c.method(node, m) for m in node_methods)
        + 'public int getId(){return this instanceof core.game.node.entity.npc.NPC ? ((core.game.node.entity.npc.NPC) this).getId() : '
          'this instanceof core.game.node.item.Item ? ((core.game.node.item.Item) this).getId() : -1;}}')
    config = c.read(SERVER / 'core/game/system/config/ItemConfigParser.kt')
    constants = ''.join('public static String ' + name + '=' + re.search(r'const val ' + name + r' = ("[^"]+")', config).group(1) + ';'
                        for name in ['HIGH_ALCHEMY', 'LOW_ALCHEMY', 'TRADEABLE', 'TOKKUL_PRICE', 'ARCHERY_TICKET_PRICE', 'CASTLE_WARS_TICKET_PRICE'])
    out['core/game/system/config/ItemConfigParser.java'] = 'package core.game.system.config;public class ItemConfigParser {' + constants + '}'
    parser = c.read(SERVER / 'core/game/system/config/InterfaceConfigParser.kt')
    replaced = parser.replace('var reader: FileReader? = null', 'var reader: java.io.Reader? = null').replace(
        'FileReader(ServerConstants.CONFIG_PATH + "interface_configs.json")', 'java.io.StringReader(oracle.Support.config)')
    if replaced.count('oracle.Support.config') != 1 or 'java.io.Reader?' not in replaced:
        raise RuntimeError('InterfaceConfigParser text differs from the expected reader lines')
    out['core/game/system/config/InterfaceConfigParser.kt'] = replaced
    texts = [c.read(SERVER / name) for name in ['core/game/node/entity/player/link/InterfaceManager.java', 'core/game/component/InterfaceType.java',
                                                 'core/game/shops/Shop.kt', 'core/game/shops/Shops.kt', 'core/game/container/Container.java']]
    names = set(re.findall(r'Components\.(\w+_\d+)\b', ''.join(texts)))
    out['org/rs09/consts/Components.java'] = ('package org.rs09.consts;public class Components{'
                                             + ''.join(f'public static final int {name}={int(name.rsplit("_", 1)[1])};' for name in sorted(names)) + '}')
    items = set(re.findall(r'Items\.(\w+_\d+)\b', ''.join(texts) + definition))
    out['org/rs09/consts/Items.java'] = ('package org.rs09.consts;public class Items{'
                                        + ''.join(f'public static final int {name}={int(name.rsplit("_", 1)[1])};' for name in sorted(items)) + '}')
    npcs = set(re.findall(r'NPCs\.(\w+_\d+)\b', ''.join(texts)))
    out['org/rs09/consts/NPCs.java'] = ('package org.rs09.consts;public class NPCs{'
                                       + ''.join(f'public static final int {name}={int(name.rsplit("_", 1)[1])};' for name in sorted(npcs)) + '}')
    return out


def pinned_json():
    pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))
    candidates = [RUN / 'json-simple-1.1.1.jar'] + sorted((ROOT / 'runs').glob('*-oracle/json-simple-1.1.1.jar'))
    for jar in candidates:
        if jar.is_file() and hashlib.sha256(jar.read_bytes()).hexdigest() == pin['sha256']:
            return jar
    raise RuntimeError('No json-simple-1.1.1.jar matching oracle/auth/json.lock.json under runs/; this generator never downloads')


# --------------------------------------------------------------------- corpus

INT_MAX = (1 << 31) - 1


def item(name, value=1, **kw):
    spec = dict(name=name, value=value, tradeable=True, examine=f'{name} examine.')
    spec.update(kw)
    return spec


FILLER = list(range(1100, 1142))
ITEMS = {
    995: item('Coins', stackable=True),
    6529: item('Tokkul', stackable=True),
    1464: item('Archery ticket', stackable=True),
    4067: item('Castle wars ticket', stackable=True),
    6306: item('Trading sticks', stackable=True),
    1931: item('Pot', noteId=1932),
    1932: item('Pot', unnoted=False, noteId=1931),
    590: item('Tinderbox'),
    1925: item('Bucket', 2, noteId=1926),
    1926: item('Bucket', 2, unnoted=False, noteId=1925),
    1351: item('Bronze axe', 16, noteId=1352),
    1352: item('Bronze axe', 16, unnoted=False, noteId=1351),
    1265: item('Bronze pickaxe'),
    558: item('Mind rune', 3, stackable=True),
    554: item('Fire rune', 4, stackable=True, tokkul_price=6),
    4151: item('Abyssal whip', 120001, high_alchemy=72000, low_alchemy=48000),
    1038: item('Red partyhat'),
    6570: item('Fire cape', 60000, tradeable=False, tokkul_price=8000),
    6571: item('Uncut onyx', 300000, tokkul_price=200000),
    4045: item('Explosive potion', options=['Drink', None, None, None, 'Destroy']),
    11238: item('Baby impling jar', 50, options=['Loot', None, None, None, 'Destroy']),
    12183: item('Spirit shards', 25, stackable=True),
    12155: item('Pouch'),
    9762: item('Magic cape', 99000),
    9763: item('Magic cape(t)', 99000),
    892: item('Rune arrow', 400, stackable=True, archery_ticket_price=250),
    4513: item('Castlewars hood', 1, castle_wars_ticket_price=30),
    2: item('Cannonball', 5, stackable=True),
    1: item('Toolkit', 0),
    13000: item('Gold statue', 2000000000),
    1323: item('Iron scimitar', 112),
    1333: item('Rune scimitar', 25600, archery_ticket_price=-1),
    7462: item('Barrows gloves', 130000, tradeable=False),
}
for number, filler in enumerate(FILLER):
    ITEMS[filler] = item(f'Filler {number}', 10 + number)


def row(id, title, stock, npcs='', general=False, currency=995, high_alch='0', **extra):
    out = dict(id=str(id), title=title, stock=stock, npcs=npcs, general_store='true' if general else 'false', currency=str(currency),
               high_alch=high_alch)
    out.update(extra)
    return out


def stock(*rows):
    return '-'.join('{' + ','.join(str(x) for x in r) + '}' for r in rows)


BASIC = [
    row(1, 'General Store', stock((1931, 5), (590, 2), (1925, 3), (1265, 0), (558, 100, 10)), '520,521', general=True),
    row(2, "Bob's Brilliant Axes", stock((1351, 10), (1265, 5, 50), (1323, 2, 1)), '519'),
    row(3, 'TzHaar-Hur-Tel', stock((554, 1000), (6571, 1), (6570, 0)), '2620', currency=6529),
    row(4, 'Archery tickets', stock((892, 500), (1333, 2)), '1032', currency=1464),
    row(5, 'Castle wars', stock((4513, 10)), '1530', currency=4067),
    row(6, 'High alch shop', stock((4151, 1), (1323, 10)), '600', high_alch='1'),
    row(7, 'Fixed prices', stock((12183, 1000), (12155, 100), (9762, 1), (9763, 1)), '601'),
    row(8, 'Noted stock', stock((1932, 10), (590, 5)), '602'),
    row(9, 'Shared general', stock((1931, 10)), '603', general=True, force_shared='true'),
    row(10, 'Trading sticks', stock((1925, 20), (554, 30)), '604', currency=6306),
    row(11, 'Restock rates', stock((590, 5, 0), (1931, 10, 1), (1925, 3, 3), (1265, 2, 2), (558, 50, 7)), '605'),
    row(12, 'Expensive', stock((13000, 5), (2, 1000), (1, 4)), '606'),
    row(13, 'Big shop', stock(*[(f, 1 + i) for i, f in enumerate(FILLER[:41])]), '607'),
    row(14, 'Duplicates', stock((1925, 5), (1925, 3), (590, 1, 25)), '608,519'),
    row(15, 'Specials', stock((590, 5)), '2824,7601,933,1316,562,1041,804'),
    row(56, 'Candles after the crystal', stock((590, 1)), ''),
    row(16, 'Second general', stock((1265, 4)), '609', general=True),
]


def P(name, uid, inv=(), **kw):
    spec = dict(name=name, uid=uid, inv=[list(r) for r in inv])
    spec.update(kw)
    return spec


def a(op, **kw):
    return dict(op=op, **kw)


def btn(p, iface, opcode, button=0, slot=0, item=-1):
    return a('button', p=p, iface=iface, opcode=opcode, button=button, slot=slot, item=item)


AMOUNT = {1: 196, 5: 124, 10: 199, 'x': 234}


def open_(p, shop):
    return a('open', p=p, shop=shop)


def buy(p, slot, amount=1, button=23):
    return btn(p, 620, AMOUNT[amount], button, slot)


def value(p, slot, button=23):
    return btn(p, 620, 155, button, slot)


def examine(p, slot, button=23):
    return btn(p, 620, 9, button, slot)


def sell(p, slot, amount=1):
    return btn(p, 621, AMOUNT[amount], 0, slot)


def sellvalue(p, slot):
    return btn(p, 621, 155, 0, slot)


def tab(p, main):
    return btn(p, 620, 155, 25 if main else 26)


def text(p, value):
    return a('input', p=p, text=value)


def number(p, value):
    return a('input', p=p, int=value)


def tick(n):
    return a('tick', n=n)


def price(p, slot, sell=False):
    return a('price', p=p, slot=slot, sell=sell)


RICH = [(0, 995, 100000), (1, 6529, 100000), (2, 1464, 100000), (3, 4067, 1000), (4, 6306, 1000)]
POOR = [(0, 995, 3)]


def file_name(text):
    return hashlib.sha256(text.encode()).hexdigest()[:16]


class Corpus:
    """Cases name their shops.json file and item table; both are stored once in inputs.json."""

    def __init__(self):
        self.cases = []
        self.files = {}
        self.items = {'basic': {str(k): v for k, v in ITEMS.items()}}

    def case(self, actions, players=None, shops=BASIC, table='basic', **kw):
        if shops == 'pinned':
            text, name = None, 'pinned'
        elif isinstance(shops, str):
            text = shops
            name = file_name(text)
        else:
            text = json.dumps(shops, indent=1)
            name = file_name(text)
        if text is not None:
            self.files[name] = text
        row = dict(itemsTable=table, ticks=kw.pop('ticks', 1), serverName=kw.pop('serverName', '2009scape'),
                   stockClear=kw.pop('stockClear', 180), recirculate=kw.pop('recirculate', True), shopsFile=name,
                   players=players or [P('alice', 11, RICH)], actions=actions)
        row.update(kw)
        self.cases.append(row)


def loader_cases(k):
    """Shops.startup and parseStock over authored rows, including every exception point."""
    def one(stock_text, **extra):
        k.case([], shops=[row(1, 'Edge', stock_text, '700', **extra)])

    for text in ['{1931,5}-{1931,3}', '{1931,5}-{590,2}-{1931,-4}', '{1931,inf}', '{1931, inf }', '{abc,5}', '{1931}', '', '{1931, 5 , x}',
                 '{ 1931,5}', '{1931,5,+7}', '{1931,5,-3}', '{1931,5, 7}', '{1931,5}-', '-{1931,5}', '{1931,5,7,9}', '{1931,2147483647}-{1931,1}',
                 '{1931,5}-{1931,x}', '1931,5', '{{1931,5}}', '{1931,Ù¥}', '{1931,+5}', '{1931,5,99999999999}']:
        one(text)
    for npcs in ['1, 2', ' ', '', '5,5', '5,,6', '-1', '2147483648']:
        k.case([], shops=[row(1, 'Npcs', stock((590, 1)), npcs)])
    for general in ['TRUE', 'yes', 'True ', 'false']:
        k.case([], shops=[dict(row(1, 'General', stock((590, 1)), '701'), general_store=general)])
    for extra in [dict(high_alch='01'), dict(high_alch='1'), dict(currency='abc'), dict(currency=' 995'), dict(id='x'), dict(force_shared='True'),
                  dict(force_shared='yes'), dict(force_shared=True), dict(currency=6529), dict(id=7)]:
        base = row(1, 'Fields', stock((590, 1)), '702')
        base.update(extra)
        k.case([], shops=[base])
    missing = row(1, 'Missing', stock((590, 1)), '703')
    for key in ['title', 'npcs', 'stock', 'general_store', 'currency', 'high_alch', 'id']:
        broken = dict(missing)
        del broken[key]
        k.case([], shops=[broken])
    k.case([], shops=[dict(row(1, 'Numbers', stock((590, 1)), '704'), stock=5)])
    k.case([], shops=[dict(row(1, 'Null title', stock((590, 1)), '704'), title=None)])
    k.case([], shops=[row(1, 'First', stock((590, 1)), '705'), row(1, 'Second', stock((1931, 2)), '706')])
    k.case([], shops=[row(1, 'First', stock((590, 1)), '705,706'), row(2, 'Second', stock((1931, 2)), '706')])
    k.case([], shops='{}')
    k.case([], shops='[1]')
    k.case([], shops='[]')
    k.case([], shops='[{"id":"1"')
    k.case([open_(0, 1)], shops='[]')
    k.case([], shops=BASIC, serverName='')
    many = [row(i, f'Shop {i}', stock((590, i)), str(800 + i)) for i in range(1, 60)]
    k.case([], shops=many)


def basic_cases(k):
    rich = [P('alice', 11, RICH)]
    two = [P('alice', 11, RICH), P('bob', 22, RICH)]
    # Opening, the two tabs, the side interface, closing.
    k.case([open_(0, 1)])
    k.case([open_(0, 1), open_(0, 1)])
    k.case([open_(0, 1), open_(0, 2), a('close', p=0)])
    k.case([open_(0, 99)])
    k.case([open_(0, 2), tab(0, False), tab(0, True), tab(0, True), tab(0, False), btn(0, 620, 155, 27), btn(0, 620, 155, 29), a('close', p=0)])
    k.case([open_(0, 1), tab(0, True), tab(0, True), buy(0, 0), a('close', p=0), open_(0, 1), buy(0, 0)])
    k.case([open_(0, 1), a('logout', p=0)])
    k.case([buy(0, 0), sell(0, 0), tab(0, False)])
    k.case([open_(0, 1), a('close', p=0), a('close', p=0), buy(0, 0)])
    k.case([open_(0, 1), a('attr', p=0, key='shop', value=None), buy(0, 0), sell(0, 0), a('close', p=0)])
    k.case([open_(0, 1), btn(0, 620, 155, 99, 0), btn(0, 620, 64, 23, 0), btn(0, 621, 64, 0, 0), btn(0, 548, 155, 0, 0)])
    # The NPC options, the specials and the destination override.
    for npc in [520, 521, 519, 2620, 700, 2824, 1041, 804, 7601, 609]:
        k.case([a('npc', p=0, npc=npc), a('npc', p=0, npc=npc, option='shop')])
    for points in [39, 40]:
        k.case([a('npc', p=0, npc=933)], [P('alice', 11, RICH, questPoints=points)])
    for stage in [0, 99, 100]:
        k.case([a('npc', p=0, npc=1316)], [P('alice', 11, RICH, quests={'THE_FREMENNIK_TRIALS': stage})])
    for stage in [0, 60, 61]:
        k.case([a('npc', p=0, npc=562)], [P('alice', 11, RICH, quests={'MERLINS_CRYSTAL': stage})])
    k.case([a('npc', p=0, npc=933, option='shop')], [P('alice', 11, RICH, questPoints=0)])
    for booth in [None, True, False]:
        for direction in ['NORTH', 'SOUTH_WEST', 'EAST']:
            k.case([a('dest', npc=520, booth=booth, dir=direction, x=3200, y=3210, z=3)])
    k.case([a('dest', npc=520, booth=True, dir='NORTH_EAST', x=-1, y=INT_MAX, z=-1)])
    # Buying every amount on each kind of shop.
    for shop, slots in [(1, range(5)), (2, range(3)), (3, range(3)), (4, range(2)), (5, range(1)), (6, range(2)), (7, range(4)), (8, range(2)),
                        (10, range(2)), (12, range(3)), (13, [0, 40]), (14, range(2))]:
        for slot in slots:
            k.case([open_(0, shop), value(0, slot), examine(0, slot), buy(0, slot, 1), buy(0, slot, 5), buy(0, slot, 10), value(0, slot)], rich)
    for value_text in ['3', '0', '-1', '1k', '2k', '1m', '2147483647', '2147483648', 'abc', '', 'Ù¡Ù¢', '05', '+5', '1K', '1kk']:
        k.case([open_(0, 1), buy(0, 0, 'x'), text(0, value_text), buy(0, 4, 'x'), text(0, value_text)], rich)
    for value_int in [0, -1, 1, 7, INT_MAX]:
        k.case([open_(0, 1), buy(0, 4, 'x'), number(0, value_int)], rich)
    k.case([open_(0, 1), buy(0, 0, 'x'), a('close', p=0), text(0, '2')], rich)
    k.case([open_(0, 1), buy(0, 0, 'x'), open_(0, 2), text(0, '2')], rich)
    k.case([open_(0, 1), buy(0, 0, 'x'), buy(0, 1, 'x'), text(0, '2')], rich)
    k.case([text(0, '2')], rich)
    # Price boundaries over stock levels (getGPCost and getGPSell).
    for shop, slot, count in [(1, 0, 6), (1, 2, 4), (2, 0, 11), (12, 1, 12), (6, 1, 11), (10, 0, 21), (3, 0, 3)]:
        steps = [open_(0, shop)]
        for _ in range(count):
            steps += [price(0, slot), buy(0, slot)]
        steps += [price(0, slot)]
        k.case(steps, rich)
    for shop, amount in [(2, 10), (2, 11), (12, 1), (12, 2), (12, 5), (1, 3), (11, 4)]:
        k.case([open_(0, shop), buy(0, 0, 'x'), text(0, str(amount)), price(0, 0)], rich)
    # Direct buy/sell amounts and statuses.
    for amount in [0, -5, 1, 3, INT_MAX, -2147483648]:
        k.case([open_(0, 1), a('buy', p=0, slot=0, amount=amount), a('sell', p=0, slot=5, amount=amount)],
               [P('alice', 11, RICH + [(5, 1931, 1), (6, 1931, 1)])])
    k.case([open_(0, 1), a('buy', p=0, slot=7, amount=1), a('buy', p=0, slot=-1, amount=1), a('sell', p=0, slot=20, amount=1)], rich)
    k.case([open_(0, 1), a('attr', p=0, key='shop-cont', value=None), a('buy', p=0, slot=0, amount=1), a('price', p=0, slot=0),
            a('price', p=0, slot=0, sell=True), value(0, 0)], rich)
    k.case([open_(0, 1), a('attr', p=0, key='shop-main', value=True), a('buy', p=0, slot=0, amount=1)], rich)
    # Money, space and stock limits.
    k.case([open_(0, 2), buy(0, 0), buy(0, 1, 10)], [P('alice', 11, POOR)])
    k.case([open_(0, 2), buy(0, 0)], [P('alice', 11, [(0, 995, 16)])])
    k.case([open_(0, 2), buy(0, 0)], [P('alice', 11, [(0, 995, 21)])])
    k.case([open_(0, 2), buy(0, 0, 5)], [P('alice', 11, [(0, 995, 100)])])
    full = [(s, 1038, 1) for s in range(1, 28)]
    k.case([open_(0, 2), buy(0, 0), buy(0, 0, 5)], [P('alice', 11, [(0, 995, 1000)] + full)])
    k.case([open_(0, 2), buy(0, 0)], [P('alice', 11, [(0, 995, 21)] + full)])
    k.case([open_(0, 1), buy(0, 4, 10), buy(0, 4, 'x'), text(0, '500')], [P('alice', 11, [(0, 995, 1000)] + full)])
    k.case([open_(0, 1), buy(0, 3)], rich)
    k.case([open_(0, 2), buy(0, 0, 'x'), text(0, '10'), buy(0, 0), price(0, 0)], rich)
    k.case([open_(0, 12), buy(0, 0), buy(0, 0, 'x'), text(0, '2'), buy(0, 0, 5)], [P('alice', 11, [(0, 995, INT_MAX)])])
    k.case([open_(0, 12), buy(0, 2), buy(0, 2, 5), sell(0, 1), sell(0, 1, 5)], [P('alice', 11, [(0, 995, 100)])])
    k.case([open_(0, 4), buy(0, 0, 5), buy(0, 1)], [P('alice', 11, [(0, 1464, 1000)])])
    k.case([open_(0, 3), buy(0, 0, 10), buy(0, 1)], [P('alice', 11, [(0, 6529, 50)])])
    # Selling: currencies, untradeables, unstocked, notes, every amount, space.
    goods = [(0, 995, 50), (1, 1931, 1), (2, 1931, 1), (3, 1931, 1), (4, 1932, 7), (5, 590, 1), (6, 558, 400), (7, 4045, 1), (8, 11238, 1),
             (9, 6570, 1), (10, 6529, 9), (11, 1464, 9), (12, 4067, 9), (13, 1351, 1), (14, 1352, 12), (15, 4151, 1), (16, 12183, 30),
             (17, 9762, 1), (18, 13000, 3), (19, 1, 2), (20, 1323, 1), (21, 7462, 1), (22, 1038, 1), (23, 892, 40), (24, 4513, 2), (25, 6571, 1),
             (26, 554, 77), (27, 1333, 1)]
    for shop in [1, 2, 3, 4, 5, 6, 7, 8, 10, 12]:
        for slot in range(28):
            k.case([open_(0, shop), sellvalue(0, slot), sell(0, slot)], [P('alice', 11, goods)])
    for shop, slot in [(1, 1), (1, 4), (1, 6), (2, 13), (2, 14), (6, 15), (6, 20), (7, 16), (8, 1), (8, 4), (1, 14), (12, 18), (3, 26), (3, 25)]:
        for amount in [5, 10, 'x']:
            steps = [open_(0, shop), sell(0, slot, amount)]
            if amount == 'x':
                steps += [text(0, '6')]
            k.case(steps + [sellvalue(0, slot), price(0, slot, True)], [P('alice', 11, goods)])
    k.case([open_(0, 1), sell(0, 22), sell(0, 21), tab(0, True), sell(0, 20), sell(0, 1), buy(0, 0, button=24), buy(0, 1, 10, button=24)],
           [P('alice', 11, goods)])
    k.case([open_(0, 1), sellvalue(0, 27), btn(0, 621, 155, 0, 28), sell(0, 28), sell(0, -1)], [P('alice', 11, goods[:27])])
    k.case([open_(0, 1), sell(0, 6, 'x'), text(0, '1000'), sell(0, 1, 'x'), number(0, 2)], [P('alice', 11, goods)])
    runes_full = [(0, 558, 50)] + [(s, 1038, 1) for s in range(1, 28)]
    k.case([open_(0, 1), sell(0, 0), sell(0, 0, 'x'), text(0, '50')], [P('alice', 11, runes_full)])
    k.case([open_(0, 1), sell(0, 1)], [P('alice', 11, [(0, 995, 5)] + [(s, 1931, 1) for s in range(1, 28)])])
    k.case([open_(0, 2), sell(0, 1)], [P('alice', 11, [(0, 1323, 1), (1, 1351, 1)] + [(s, 1038, 1) for s in range(2, 28)])])
    # Overstock and decay of sell prices; buying back from player stock.
    k.case([open_(0, 1)] + [sell(0, 1)] * 14 + [price(0, 0), buy(0, 0, 10), price(0, 0)],
           [P('alice', 11, [(0, 995, 10)] + [(s, 1931, 1) for s in range(1, 28)])])
    k.case([open_(0, 2)] + [sell(0, 0, 10)] * 3 + [price(0, 0, True), sell(0, 0, 'x'), text(0, '15'), price(0, 0, True)],
           [P('alice', 11, [(0, 1352, 60)])])
    k.case([open_(0, 1), sell(0, 0, 10), tab(0, False), price(0, 0), buy(0, 0, 5, button=24), price(0, 0), buy(0, 0, 10, button=24), tab(0, True)],
           [P('alice', 11, [(0, 1038, 1), (1, 995, 100)] + [(s, 1038, 1) for s in range(2, 12)])])
    k.case([open_(0, 1), sell(0, 0), open_(0, 16), tab(0, False), buy(0, 0, button=24), open_(0, 9), tab(0, False), sell(0, 3)],
           [P('alice', 11, [(0, 1038, 1), (1, 995, 100), (3, 1323, 1)])])
    general_full = [(s, f, 1) for s, f in enumerate(FILLER[:28])]
    k.case([open_(0, 1)] + [sell(0, s) for s in range(28)], [P('alice', 11, general_full)])
    k.case([open_(0, 1)] + [sell(0, s) for s in range(28)] + [a('invReplace', p=0, slot=s, id=FILLER[28 + s], amount=1) for s in range(13)]
           + [sell(0, s) for s in range(13)], [P('alice', 11, general_full)])
    # Overstock beyond the cap of 10, one at a time and in one multi-unit sale.
    pots = [(0, 995, 10)] + [(s, 1931, 1) for s in range(1, 21)]
    steps = [open_(0, 1)]
    for s in range(1, 19):
        steps += [price(0, s, True), sell(0, s)]
    k.case(steps + [price(0, 0), buy(0, 0, 10), price(0, 0)], [P('alice', 11, pots)])
    k.case([open_(0, 1), sell(0, 1, 'x'), text(0, '300'), price(0, 1, True), sell(0, 1, 'x'), text(0, '7'), price(0, 1, True)],
           [P('alice', 11, [(0, 995, 10), (1, 558, 400)])])
    k.case([open_(0, 2), sell(0, 0, 'x'), text(0, '25'), price(0, 0, True), sell(0, 0, 10), price(0, 0, True)],
           [P('alice', 11, [(0, 1352, 60)])])
    k.case([open_(0, 6), sell(0, 0, 'x'), text(0, '30'), price(0, 0, True)], [P('alice', 11, [(0, 1323, 1)] + [(s, 1323, 1) for s in range(1, 25)])])
    # A stock that holds an item and its note: getStockSlot keeps the last match.
    for rows in [((1931, 5), (1932, 3), (590, 2)), ((1932, 3), (1931, 5), (590, 2)), ((1351, 5), (1352, 3), (590, 2)), ((1352, 3), (1351, 5), (590, 2))]:
        notes = BASIC + [row(17, 'Notes both', stock(*rows), '610')]
        k.case([open_(0, 17), price(0, 0), price(0, 1), sell(0, 1), sell(0, 2, 'x'), text(0, '4'), price(0, 1, True), price(0, 2, True),
                buy(0, 0), buy(0, 1), tick(5)], [P('alice', 11, [(0, 995, 1000), (1, rows[0][0], 1), (2, rows[0][0] if rows[0][0] in (1932, 1352) else rows[1][0], 5)])],
               shops=notes)
        k.case([open_(0, 17), sell(0, 1), sell(0, 3), sell(0, 4), sell(0, 2, 'x'), text(0, '6'), price(0, 0), price(0, 1), price(0, 5, True)],
               [P('alice', 11, [(0, 995, 1000), (1, 1351, 1), (2, 1352, 9), (3, 1351, 1), (4, 1351, 1), (5, 1352, 1)])], shops=notes)
    # Noted stock: the note-change slot and the stock array beyond its size.
    k.case([open_(0, 8), sell(0, 0), buy(0, 2), price(0, 2), value(0, 2), tick(1)], [P('alice', 11, [(0, 1931, 1), (1, 995, 100)])])
    k.case([open_(0, 8), sell(0, 0, 10), price(0, 0), buy(0, 0), tick(3)], [P('alice', 11, [(0, 1932, 20), (1, 995, 100)])])
    # Item views supplied by content.
    k.case([a('openView', p=0, shop=1, hide=[1931]), value(0, 0), buy(0, 0), value(0, 1), buy(0, 1), examine(0, 0), price(0, 0), tick(1)], rich)
    k.case([a('openView', p=0, shop=1, swap=[590, 1925]), value(0, 1), buy(0, 1), price(0, 1), tab(0, False), tab(0, True)], rich)
    k.case([a('openView', p=0, shop=1, swap=[590, 1925]), open_(0, 1), buy(0, 1)], rich)
    k.case([a('openView', p=0, shop=77, hide=[])], rich)
    # Ironmen: player stock, overstock, personalized.
    for mode in ['STANDARD', 'HARDCORE', 'ULTIMATE', 'NONE']:
        iron = P('alice', 11, [(0, 995, 1000), (1, 1931, 1), (2, 1931, 1), (3, 1038, 1)], ironman=mode)
        k.case([open_(0, 1), sell(0, 3), buy(0, 0, button=24), sell(0, 1), sell(0, 2), tab(0, True), buy(0, 0)], [iron])
        k.case([open_(0, 1), sell(0, 3), buy(0, 0, button=24), sell(0, 1), sell(0, 2), tab(0, True), buy(0, 0)], [iron], personalized=True)
    k.case([open_(0, 9), sell(0, 1), sell(0, 2), tab(0, True), buy(0, 0)],
           [P('alice', 11, [(0, 995, 1000), (1, 1931, 1), (2, 1931, 1)], ironman='STANDARD')], personalized=True)
    k.case([open_(0, 1), sell(0, 1), a('ironman', p=0, mode='STANDARD'), buy(0, 0), a('ironman', p=0, mode='NONE'), buy(0, 0)],
           [P('alice', 11, [(0, 995, 1000), (1, 1931, 1)])])
    # Two players: shared stock, listeners, updates to both, uid collisions.
    k.case([open_(0, 1), open_(1, 1), buy(0, 0), sell(1, 5), tab(1, False), buy(1, 0, button=24), a('close', p=0), buy(1, 0)],
           [P('alice', 11, RICH), P('bob', 22, RICH + [(5, 1038, 1)])])
    k.case([open_(0, 1), open_(1, 1), buy(0, 0), tick(2), a('close', p=1), buy(0, 0)], two, personalized=True)
    k.case([open_(0, 1), open_(1, 1), buy(0, 0), a('close', p=0), buy(1, 0), a('close', p=1)], [P('alice', 11, RICH), P('bob', 11, RICH)])
    k.case([open_(0, 1), open_(1, 2), open_(0, 2), buy(1, 0), a('close', p=1), buy(0, 0)], [P('alice', 11, RICH), P('bob', 11, RICH)])
    k.case([open_(0, 1), open_(1, 9), sell(0, 5), tab(1, False), sell(1, 5), tick(1)],
           [P('alice', 11, RICH + [(5, 1038, 1)]), P('bob', 22, RICH + [(5, 1323, 1)])])
    # Personalized shops and switching the setting while shops exist.
    k.case([open_(0, 2), buy(0, 0, 5), open_(1, 2), buy(1, 0), tick(100), price(0, 0), price(1, 0)], two, personalized=True)
    k.case([open_(0, 9), buy(0, 0, 5), open_(1, 9), buy(1, 0)], two, personalized=True)
    k.case([open_(0, 2), a('config', personalized=False), a('close', p=0)], two, personalized=True)
    k.case([open_(0, 2), a('config', personalized=True), a('close', p=0), open_(0, 2), buy(0, 0)], two)
    k.case([open_(0, 2), a('config', personalized=True), buy(0, 0), tick(50)], two)
    k.case([open_(0, 2), buy(0, 0), a('config', personalized=False), tick(50), a('close', p=0)], two, personalized=True)
    k.case([open_(0, 2), buy(0, 0)], two, personalized=False)
    k.case([open_(0, 2), buy(0, 0, 5), tick(10)], two, personalized=True, serverName='')
    for name in ['2009scape', '', 'Aa', 'BB', 'polygenelubricants', 'Ã©']:
        k.case([open_(0, 1), buy(0, 0), tick(2)], rich, serverName=name)


def restock_cases(k):
    rich = [P('alice', 11, RICH)]
    # Restocking and decay at each rate, over long runs.
    k.case([open_(0, 11)] + [buy(0, s, 10) for s in range(5)] + [tick(1), tick(2), tick(3), tick(7), tick(100), tick(1000)], rich)
    k.case([open_(0, 11)] + [buy(0, s, 10) for s in range(5)] + [a('close', p=0), tick(5000)], rich)
    k.case([open_(0, 11)] + [buy(0, s, 10) for s in range(5)] + [tick(1), tick(1), tick(1)], rich, ticks=-3)
    k.case([open_(0, 11)] + [buy(0, s, 10) for s in range(5)] + [tick(9)], rich, ticks=INT_MAX - 4)
    over = [(0, 995, 10)] + [(s, 1931, 1) for s in range(1, 21)] + [(21, 590, 1), (22, 1925, 1), (23, 558, 300)]
    k.case([open_(0, 11)] + [sell(0, s) for s in range(1, 24)] + [tick(1), tick(10), tick(100), tick(400)], [P('alice', 11, over)])
    k.case([open_(0, 1), buy(0, 0, 5), buy(0, 4, 10), tick(99), tick(1), tick(1000), price(0, 0)], rich)
    k.case([open_(0, 1), buy(0, 0, 5), a('close', p=0), tick(1000), open_(0, 1)], rich)
    k.case([open_(0, 2), buy(0, 0, 10), buy(0, 1, 5), buy(0, 2, 1), tick(1), tick(49), tick(1), tick(500)], rich)
    k.case([open_(0, 2), buy(0, 0, 10), open_(1, 2), tab(0, False), tick(100), tab(0, True), tick(100)], [P('alice', 11, RICH), P('bob', 22, RICH)])
    k.case([open_(0, 1), buy(0, 0, 5), tick(300)], rich, personalized=True)
    k.case([open_(0, 1), buy(0, 0, 5), open_(1, 1), buy(1, 1), tick(300)], [P('alice', 11, RICH), P('bob', 22, RICH)], personalized=True)
    k.case([open_(0, 13)] + [buy(0, s, 10) for s in range(0, 41, 5)] + [tick(250)], rich)
    k.case([open_(0, 14), buy(0, 0, 5), buy(0, 1), tick(25), tick(25), tick(100)], rich)
    # The general player stock clear and its recirculation.
    general = [(0, 995, 10), (1, 1038, 1), (2, 1323, 1), (3, 7462, 1), (4, 4151, 1)]
    for minutes, start in [(1, 1), (1, 0), (1, 95), (2, 1), (0, 1), (-1, 1), (180, 17990), (40000, 1), (35792, -50)]:
        for recirculate in [True, False]:
            k.case([open_(0, 1), sell(0, 1), sell(0, 2), sell(0, 4), tab(0, False), tick(5), tick(100), tick(300)],
                   [P('alice', 11, general)], stockClear=minutes, ticks=start, recirculate=recirculate)
    k.case([open_(0, 1), sell(0, 1), tick(1)], [P('alice', 11, general)], stockClear=1, ticks=100, failures={'geOffer': 1})
    # Injected failures mid-operation.
    for event, at in [('send', 20), ('send', 30), ('config', 25), ('message', 1), ('contentMessage', 1), ('invUpdate', 1),
                      ('purchase', 1), ('sale', 1), ('examine', 1), ('dialogue', 1), ('log', 1), ('log', 2), ('sendInput', 1), ('debug', 1),
                      ('view', 1), ('listen', 2), ('buttonClick', 2)]:
        k.case([a('openView', p=0, shop=1, hide=[]), value(0, 0), buy(0, 0), examine(0, 0), sell(0, 5), sellvalue(0, 5), sell(0, 6), sell(0, 9),
                btn(0, 621, 196, 0, 20), buy(0, 1, 'x'), text(0, 'x'), a('close', p=0), tick(3), open_(0, 1), buy(0, 0)],
               [P('alice', 11, RICH + [(5, 1038, 1), (6, 1323, 1), (9, 995, 3)])], failures={event: at})


def pinned_cases(k):
    shops = json.loads(c.read(PINNED))
    ids = set()
    for entry in shops:
        for part in [p for p in entry['stock'].split('-') if p]:
            ids.add(int(part.strip('{}').split(',')[0]))
    items = {995: item('Coins', stackable=True), 6529: item('Tokkul', stackable=True), 1464: item('Archery ticket', stackable=True),
             4067: item('Castle wars ticket', stackable=True), 6306: item('Trading sticks', stackable=True)}
    currency = {}
    for entry in shops:
        for part in [p for p in entry['stock'].split('-') if p]:
            currency.setdefault(int(part.strip('{}').split(',')[0]), entry['currency'])
    for id in sorted(ids):
        if id in items:
            continue
        spec = item(f'Item {id}', (id * 7919) % 20000 + 1)
        cur = currency[id]
        if cur == '6529':
            spec['tokkul_price'] = (id * 31) % 900 + 1
        elif cur == '1464':
            spec['archery_ticket_price'] = (id * 17) % 300 + 1
        elif cur == '4067':
            spec['castle_wars_ticket_price'] = (id * 13) % 80 + 1
        items[id] = spec
    k.items['pinned'] = {str(key): value for key, value in items.items()}
    purse = [(0, 995, 50000000), (1, 6529, 5000000), (2, 1464, 5000000), (3, 4067, 5000000), (4, 6306, 5000000)]
    ordered = [int(entry['id']) for entry in shops]
    chunk = 25
    for start in range(0, len(ordered), chunk):
        steps = []
        for shop in ordered[start:start + chunk]:
            steps += [open_(0, shop), value(0, 0), buy(0, 0), sellvalue(0, 5), sell(0, 5), buy(0, 1, 5), a('close', p=0),
                      a('invReplace', p=0, slot=5, id=None), a('invReplace', p=0, slot=6, id=None)]
        steps += [tick(1), tick(500)]
        k.case(steps, [P('alice', 11, purse)], shops='pinned', table='pinned', ticks=1)
    k.case([open_(0, s) for s in ordered[:40]] + [a('npc', p=0, npc=n) for n in [520, 521, 2620, 1316]] + [tick(10)],
           [P('alice', 11, purse)], shops='pinned', table='pinned', personalized=True)
    k.case([], [P('alice', 11, purse)], shops='pinned', table='pinned', personalized=True)


def corpus():
    k = Corpus()
    loader_cases(k)
    basic_cases(k)
    restock_cases(k)
    pinned_cases(k)
    return k


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
    pinned = c.read(PINNED)

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

    k = corpus()
    cases_dir = ROOT / CASES_DIR
    if cases_dir.exists():
        shutil.rmtree(cases_dir)
    for name, text in sorted(k.files.items()):
        (cases_dir / name).mkdir(parents=True)
        (cases_dir / name / 'shops.json').write_bytes(text.encode('utf-8'))
    (cases_dir / 'pinned').mkdir(parents=True)
    (cases_dir / 'pinned' / 'shops.json').write_bytes(pinned.encode('utf-8'))
    cases = k.cases
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(dict(case, items=k.items[case['itemsTable']])) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx2g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', f'-Dshops.config={CONFIG}',
               f'-Dshops.dir={CASES_DIR}', '-cp', launcher, 'oracle.Launcher', classes, *libraries]
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=7200)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-500:])
    outputs = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(outputs) != len(cases):
        raise RuntimeError(f'{len(outputs)} results for {len(cases)} cases')
    rows = [dict(case, expected=output['expected']) for case, output in zip(cases, outputs)]
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    inputs = (json.dumps(dict(files=k.files, items=k.items), sort_keys=True, indent=1) + '\n').encode()
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
    constructed = sum(len(row['expected'][0]['result']['shops']) for row in rows if row['expected'][0]['result'])
    pinned_setups = sum(1 for row in rows if row['shopsFile'] == 'pinned' and row['expected'][0]['result'])
    ticks = sum(action['n'] for row in rows for action in row['actions'] if action['op'] == 'tick')
    coverage = dict(cases=len(rows), actions=sum(operations.values()), operations=dict(sorted(operations.items())),
                    recordedCalls=sum(events.values()), events=dict(sorted(events.items())), errors=dict(sorted(errors.items())),
                    shopsConstructed=constructed, pinnedStartups=pinned_setups, ticks=ticks,
                    purchases=events.get('purchase', 0), sales=events.get('sale', 0),
                    injectedFailures=sum(len(row.get('failures', {})) for row in rows))
    for needed in ['send', 'message', 'contentMessage', 'dialogue', 'log', 'config', 'purchase', 'sale', 'invUpdate',
                   'sendInput', 'examine', 'geOffer', 'tanning', 'npcDialogue', 'questPoints', 'questStage', 'questComplete', 'view',
                   'npcAttribute', 'tickFailure', 'buttonClick', 'listen', 'destination', 'command', 'debug']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        interfaceConfigSha256=hashlib.sha256(config.replace(b'\r', b'')).hexdigest(),
        pinnedShopsSha256=hashlib.sha256(pinned.encode()).hexdigest(), inputsSha256=hashlib.sha256(inputs).hexdigest(), coverage=coverage,
        sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original shop cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{constructed} shops constructed, {coverage["purchases"]} purchases, {coverage["sales"]} sales, {ticks} ticks)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or (FIXTURES / 'inputs.json').read_bytes() != inputs or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Shops fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'inputs.json').write_bytes(inputs)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary} under {FIXTURES}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
