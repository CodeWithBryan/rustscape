"""Join original InteractPlugin defaults with real ItemDefinition option lookup."""
import argparse, importlib.util, itertools, os, pathlib, sys
ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('legacy', ROOT/'oracle/legacy-interaction/generate.py')
legacy = importlib.util.module_from_spec(spec); spec.loader.exec_module(legacy)
FOLDER = 'ground-options'
SCOPE = ('Complete unchanged InteractPlugin, Option, OptionHandler and PickupPlugin classes; '
         'original ItemDefinition forId/getOptionHandler/setOptionHandler/getOptionHandlers, '
         'Definition.getConfiguration, ground/inventory array getters and Item ID/charge bodies. '
         'Joined default initialization, real global/local typed handler identities, cached definitions '
         'versus ID lookups, retained arrays, ROOT locale and failed prefixes. '
         'Node/Item construction, metadata fields, profile/pickup execution and transport are adapters. '
         'No approach/arrival, authoritative Take, complete startup or content parity claim.')

def op(action, **kw): return dict(op=action, **kw)
def requests():
    rows = []
    names = [None, [], [None, None, 'Take', None, None],
             ['null', 'Take', 'TAKE', '', 'NULL', 'İΣ😀']]
    for located, values, local, glob in itertools.product([False, True], names,
                                                        [None, 0, 1, 'wrong'], [None, 0, 1]):
        rows.append(dict(located=located, names=values, local=local, globalHandler=glob,
                         actions=[op('default'), op('capture'), op('register', name='take', value=1),
                                  op('default'), op('initialized', value=False), op('default')]))
    for action in [op('charge', value=-1), op('charge', value=65536), op('cached', value=1),
                   op('cached', value=None), op('located', value=False),
                   op('names', value=['Use', 'Take']), op('names', value=None),
                   op('config', value='wrong'), op('config', value=1),
                   op('register', name='take', value=None)]:
        rows.append(dict(actions=[op('default'), op('capture'), action,
                                  op('default'), op('initialized', value=False), op('default')]))
    for name in [None, '', 'take', 'Take', 'null', 'i\u0307ς😀']:
        rows.append(dict(names=['Take', '', 'NULL', 'İΣ😀'],
                         actions=[op('default'), op('capture'), op('register', name=name, value=1),
                                  op('initialized', value=False), op('default')]))
    return rows

def transform(name, text):
    c = legacy.c
    if name == 'core/cache/def/impl/ItemDefinition.java':
        source = legacy.read(legacy.SERVER/name)
        signatures = ['public static ItemDefinition forId(int itemId)',
                      'public static OptionHandler getOptionHandler(int nodeId, String name)',
                      'public static boolean setOptionHandler(String name, OptionHandler handler)',
                      'public static Map<String, OptionHandler> getOptionHandlers()',
                      'public String[] getGroundOptions()', 'public String[] getInventoryOptions()']
        return '''package core.cache.def.impl;import java.util.*;import core.game.interaction.OptionHandler;
import core.tools.Log;import static core.api.ContentAPIKt.log;
public class ItemDefinition extends oracle.Support.Definition{
public static final Map<Integer,ItemDefinition>DEFINITIONS=new HashMap<>();
private static final Map<String,OptionHandler>OPTION_HANDLERS=new HashMap<>();
public final Map<String,Object>handlers=new HashMap<>();
''' + ''.join(c.method(source, s) for s in signatures) + c.method(
            legacy.read(legacy.SERVER/'core/cache/def/Definition.java'),
            'public <V> V getConfiguration(String key)') + '}'
    if name == 'core/game/node/item/Item.java':
        source = legacy.read(legacy.SERVER/name)
        return '''package core.game.node.item;public class Item extends core.game.node.Node{
public int idHash;public core.cache.def.impl.ItemDefinition definition;
public core.cache.def.impl.ItemDefinition getDefinition(){return definition;}
''' + ''.join(c.method(source, s) for s in ['public int getId()', 'public void setCharge(int charge)']) + '}'
    if name == 'core/game/node/entity/player/Player.java':
        return text[:-1] + '''public java.util.Map<String,Object>getAttributes(){throw new UnsupportedOperationException();}
public void removeAttribute(String s){throw new UnsupportedOperationException();}}'''
    return text

def register(adapter, read, c):
    read(pathlib.Path(__file__))
    p = legacy.SERVER/'content/global/handlers/item/PickupPlugin.java'
    read(p)
    # The complete original class is compiled unchanged; its gameplay handle body
    # is deliberately not invoked by this option-initialization corpus.
    adapter('content/global/handlers/item/PickupPlugin.java', p.read_text())
    adapter('core/plugin/Initializable.java', 'package core.plugin;public @interface Initializable{}')
    adapter('core/game/node/item/GroundItem.java', 'package core.game.node.item;public class GroundItem extends Item{}')
    adapter('core/game/global/action/PickupHandler.java', '''package core.game.global.action;
public class PickupHandler{public static boolean take(core.game.node.entity.player.Player p,core.game.node.item.GroundItem i){throw new UnsupportedOperationException();}}''')

def generate(check):
    legacy.RUN = ROOT/'runs/ground-options-oracle'
    legacy.requests = requests
    legacy.generate(check, sys.modules[__name__])

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__); p.add_argument('--check', action='store_true')
    p.add_argument('--java-home', type=pathlib.Path); a = p.parse_args()
    legacy.c.JAVA_HOME = a.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])
                                      if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(a.check)
