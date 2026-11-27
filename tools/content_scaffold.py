"""Lay out a content port (docs/content-porting.md).

    python tools/content_scaffold.py dialogue content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt
    cargo xtask content-scaffold dialogue <source>          (the same, through xtask)

`dialogue` is for a DialoguePlugin / DialogueLabeller / DialogueBuilderFile source. It writes, for topic T (the
first class's name in kebab case) and module M (snake case) in area A (the source's region folder):

    oracle/T/generate.py            the oracle: the source compiled unchanged over the dialogue content kit
    oracle/T/cases.py               an empty corpus with the actions the kit offers, ready to fill
    crates/server/src/world/content/A/M.rs
                                    the native module: the classes as PluginBody/Conversation stubs, their ids,
                                    and a `Host` trait listing the outside calls the scaffold found in the source
    crates/server/tests/content_M.rs
                                    the replay test (oracle/content-kit/dialogue.rs over the recording host)
    crates/server/src/world/live/ported/M.rs
                                    the live world's answers to the port's Host (each refuses until written)
    docs/T.md                       the port's document, with the scope and the services to fill in
and registers the port: `world/content` (module, Host bound, dialogue plugin table), live/ported (module),
oracle/ci.toml (an entry). It then runs the generator once (unless --no-generate), so the empty corpus exists
and `cargo xtask T-oracle --check` passes before any behaviour is written. Nothing is overwritten without --force.

`generic` (an InteractionListener, an option handler, anything else) writes only the native module, the docs stub
and a list of the outside calls: the kit has no shared oracle layer for those yet (see the recipe).

Every generated file is UTF-8 with LF line endings.
"""
import argparse
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
SERVER = ROOT / 'reference/server/Server/src/main'

# Members of the dialogue library a content dialogue calls without leaving it: not services.
DIALOGUE_VOCABULARY = set('''
assignToIds label goto exec manual player npc playerl npcl item item2 iteml line options input afterClose open
openDialogue loadLabel DialogueOption ChatAnim FacialExpression showTopics Topic IfTopic sendDialogue sendDialogues
interpreter end stage handle newInstance getIds intArrayOf arrayOf listOf also apply let run with takeIf
sendNormalDialogue sendOptions sendChoices finish next increment getAndIncrement close init addOption loadFile
setStage getStage npcId buttonId optInput optButton face sendInputDialogue setUnclosable splitLines
String Int Boolean println printf format toString equals hashCode
'''.split())
KOTLIN_WORDS = set('if when for while return fun class object val var else is in as null true false this super'.split())
CONSTANT_OBJECTS = {'NPCs', 'Items', 'Sounds', 'Animations', 'Graphics', 'Scenery', 'Components', 'Quests'}
# Calls content::Services already answers (crates/server/src/world/content/mod.rs): pattern -> method.
SHARED_SERVICES = {r'GameWorld\.settings\?\.name': 'settings_name'}
LIBRARY_RECEIVERS = {'ChatAnim', 'FacialExpression', 'Topic', 'IfTopic', 'DialogueOption', 'super', 'this'}


def kebab(name):
    return re.sub(r'(?<=[a-z0-9])(?=[A-Z])', '-', name).lower()


def snake(name):
    return kebab(name).replace('-', '_')


def area_of(relative):
    parts = pathlib.PurePosixPath(relative).parts
    if len(parts) > 3 and parts[:2] == ('content', 'region'):
        return parts[3]
    if len(parts) > 2 and parts[0] == 'content':
        return parts[1]
    return 'misc'


def classes(text):
    """(name, superclass) of each top-level class the source declares."""
    pattern = (r'(?m)^(?:@\w+\s+)*(?:public\s+|open\s+|final\s+|abstract\s+)*class\s+(\w+)\s*'
               r'(?:\([^)]*\))?\s*(?::\s*(\w+)|extends\s+(\w+))?')
    return [(m.group(1), m.group(2) or m.group(3)) for m in re.finditer(pattern, text)]


def ids_of(text):
    """getIds' NPC ids: (constant name, value) for NPCs.X_123, ('', value) for literals."""
    body = re.search(r'getIds\(\)[^{]*\{(.*?)\n\s*\}', text, re.S)
    if not body:
        return []
    out = [(m.group(1), int(m.group(2))) for m in re.finditer(r'NPCs\.([A-Z0-9_]+?_(\d+))\b', body.group(1))]
    return out or [('', int(v)) for v in re.findall(r'\b(\d{2,5})\b', body.group(1))]


def outside_calls(text, local):
    """Calls and property reads that look like they leave the dialogue library: candidates for Host services."""
    code = re.sub(r'"(?:\\.|[^"\\])*"', '""', text)
    code = re.sub(r'(?s)/\*.*?\*/|//[^\n]*', '', code)
    code = re.sub(r'(?m)^\s*(?:package|import)\s.*$', '', code)
    found = {}
    # Receiver-qualified: GameWorld.settings?.name, player.questRepository.getStage(...)
    for m in re.finditer(r'(?<![\w.])([A-Za-z_]\w*)((?:\s*[?!]?\.\s*[A-Za-z_]\w*(?:\([^()]*\))?)+)', code):
        receiver, chain = m.group(1), re.sub(r'\s+', '', m.group(2))
        if receiver in CONSTANT_OBJECTS or receiver in local or receiver in KOTLIN_WORDS or receiver in LIBRARY_RECEIVERS:
            continue
        first = re.match(r'[?!]*\.(\w+)', chain).group(1)
        if receiver in ('player', 'npc') and first in ('id', 'getId', 'name', 'username'):
            continue
        if receiver in ('player', 'npc', 'interpreter') and first in DIALOGUE_VOCABULARY:
            continue
        found.setdefault(receiver + chain, None)
    # Unqualified calls of functions outside the vocabulary (ContentAPI and friends).
    # Functions the source declares itself (override fun addConversation()) are not calls out.
    declared = set(re.findall(r'\b(?:fun|void|boolean|int|Object|Component)\s+(\w+)\s*\(', code))
    for m in re.finditer(r'(?<![\w.])([a-z]\w*)\s*\(', code):
        name = m.group(1)
        if name not in DIALOGUE_VOCABULARY and name not in KOTLIN_WORDS and name not in local and name not in declared:
            found.setdefault(name + '(...)', None)
    return list(found)


def method_names(calls):
    names, out = set(), []
    for call in calls:
        words = re.findall(r'[A-Z]?[a-z0-9]+|[A-Z]+(?![a-z])', call.split('(')[0])
        name = '_'.join(w.lower() for w in words) or 'service'
        while name in names:
            name += '_'
        names.add(name)
        out.append(name)
    return out


def write(path, text, force, written):
    path = ROOT / path
    if path.exists() and not force:
        sys.exit(f'{path.relative_to(ROOT)} exists (pass --force to overwrite)')
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8', newline='\n')
    written.append(path.relative_to(ROOT).as_posix())


def insert(path, marker, line, written):
    """Puts `line` before the marker line, once, at the marker's indentation."""
    path = ROOT / path
    text = path.read_text(encoding='utf-8')
    if re.sub(r'\s', '', line) in re.sub(r'\s', '', text):  # already there, perhaps reformatted
        return
    lines = text.split('\n')
    at = next((i for i, l in enumerate(lines) if l.strip() == marker), None)
    if at is None:
        sys.exit(f'{path.relative_to(ROOT)} has no "{marker}" line')
    lines.insert(at, re.match(r'\s*', lines[at]).group(0) + line)
    path.write_text('\n'.join(lines), encoding='utf-8', newline='\n')
    written.append(path.relative_to(ROOT).as_posix() + ' (registered)')


def rust_str(s):
    return s.replace('\\', '\\\\').replace('"', '\\"')


def const_name(name, value):
    return re.sub(r'_\d+$', '', name) if name else f'NPC_{value}'


def split_shared(calls):
    """(calls left for the port's Host, [(call, Services method)] already shared)."""
    left, shared = [], []
    for call in calls:
        service = next((s for pattern, s in SHARED_SERVICES.items() if re.fullmatch(pattern, call)), None)
        (shared.append((call, service)) if service else left.append(call))
    return left, shared


def native_module(relative, topic, plugin_classes, file_classes, ids, calls, shared):
    class_list = ', '.join(f'`{n}`' for n, _ in plugin_classes + file_classes)
    out = [f'//! {class_list} ({relative}).',
           '//! TODO: one sentence on what the content does.',
           '//!',
           f'//! Checked against fixtures/{topic} (oracle/{topic}); docs/{topic}.md has',
           '//! the scope and the services. Every call outside the dialogue library is',
           '//! a required [`Host`] or [`content::Services`] service with no default.',
           '#![allow(unused_imports)]',
           'use crate::world::{',
           '    content,',
           '    dialogue::{self as d, Arg, NodeHandle, Plugin, PluginBody, Result, Text},',
           '    dialogue_dsl::{',
           '        ChatAnim, Dialogue, api,',
           '        labeller::{self, Conversation, DialogueOption, Dsl},',
           '        texts,',
           '    },',
           '};',
           '']
    for name, value in ids:
        out += [f'/// `NPCs.{name}`.' if name else '/// An NPC id of getIds.', f'pub const {const_name(name, value)}: i32 = {value};']
    out += ['',
            f'/// The calls {pathlib.PurePosixPath(relative).name} makes outside the dialogue library, as the',
            '/// scaffold found them: give each its original signature, move one that other',
            '/// content shares to [`content::Services`], delete what is not a service.',
            'pub trait Host: content::Services {']
    for call, name in zip(calls, method_names(calls)):
        out += [f'    /// `{call}`.', f'    fn {name}(&mut self) -> Result<()>;']
    out += ['}', '']
    for call, service in shared:
        out.insert(out.index('pub trait Host: content::Services {'), f'/// `{call}` is already [`content::Services::{service}`].')
    const_ids = ', '.join(const_name(n, v) for n, v in ids)
    for name, _ in plugin_classes:
        out += [f'/// `{name}`, the registered prototype (ClassScanner: `{name}()`, then `init()`).',
                'pub fn prototype<H: content::Host>() -> Plugin<H> {',
                f'    Plugin::prototype({name})',
                '}', '',
                f'/// `{name}`.',
                f'pub struct {name};',
                f'impl<H: content::Host> PluginBody<H> for {name} {{',
                '    fn class_name(&self) -> &str {',
                f'        "{name}"',
                '    }',
                '    fn ids(&self, _: &mut H, _: &Plugin<H>) -> Result<Vec<i32>> {',
                f'        Ok(vec![{const_ids}])',
                '    }',
                '    fn new_instance(',
                '        &self,',
                '        host: &mut H,',
                '        _: &Plugin<H>,',
                '        player: Option<NodeHandle>,',
                '    ) -> Result<Option<Plugin<H>>> {',
                f'        Plugin::new(host, player, {name}).map(Some)',
                '    }',
                '    fn handle(',
                '        &self,',
                '        _host: &mut H,',
                '        _this: &Plugin<H>,',
                '        _interface_id: i32,',
                '        _button_id: i32,',
                '    ) -> Result<bool> {',
                f'        // TODO: {name}.handle.',
                f'        Err(d::Error::Binding("{name}.handle is not ported yet".into()))',
                '    }',
                '    fn as_any(&self) -> &dyn std::any::Any {',
                '        self',
                '    }',
                '}', '']
    for name, _ in file_classes:
        out += [f'/// `{name}`.',
                f'pub struct {name};',
                f'impl<H: content::Host> Conversation<H> for {name} {{',
                '    fn class_name(&self) -> &str {',
                f'        "{name}"',
                '    }',
                "    fn add_conversation(&self, _host: &mut H, _c: &Dsl<'_, H>) -> Result<()> {",
                f'        // TODO: {name}.addConversation.',
                f'        Err(d::Error::Binding("{name}.addConversation is not ported yet".into()))',
                '    }',
                '}', '']
    return '\n'.join(out)


def generate_py(relative, topic, found):
    names = ', '.join(n for n, _ in found)
    return f'''"""Run the original {names} ({relative}) on JDK 11 over the dialogue content kit
(oracle/content-kit/dialogue.py) and record what it does. Laid out by tools/content_scaffold.py.

The source is compiled unchanged on top of the dialogue-dsl build (its unchanged classes, inserted original text and
recording stand-ins) with the kit's stand-ins. Java/Kotlin are test oracles only; --check never rewrites a fixture.
"""
import importlib.util
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
RUN = 'runs/{topic}-oracle'  # where the kit builds (tools/run_oracles.py reads this name)


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


kit = load('content_kit_dialogue', ROOT / 'oracle/content-kit/dialogue.py')
cases = load('{snake(topic)}_cases', HERE / 'cases.py')

SOURCES = ['{relative}']
SCOPE = (
    '{relative} compiled unchanged on JDK 11 with Kotlin 1.8.20 and ConstLib 1.4.28 over the dialogue content kit: '
    'the dialogue-dsl build (its unchanged dialogue core and DSLs, inserted original text and recording stand-ins) '
    'with the kit stand-ins. Registration is ClassScanner\\'s DIALOGUE branch (the no-argument constructor, init()); '
    'talking is DialogueInterpreter.open(id, npc). TODO: what the cases cover and what they do not. '
    'Not packet bytes, real interfaces or definitions, or a live session.'
)

if __name__ == '__main__':
    kit.main('{topic}', SOURCES, cases, SCOPE, __doc__)
'''


def cases_py(topic, plugin_classes, ids, prefix):
    plugin = plugin_classes[0][0] if plugin_classes else None
    npc = ids[0][1] if ids else 0
    lines = [f'"""Cases for {topic}: what the original does, recorded by oracle/{topic}/generate.py.', '',
             '`cases()` yields (case, sweep). A case is the dialogue-dsl world (npcs, attrs, items, male, ...: see',
             "oracle/dialogue-dsl/Harness.java `world`) plus the kit's (`settings`: null for no settings, `serverName`)",
             'and its `actions`. Actions: `register` (class: the scanned name), `openId` (key, npc: an index into npcs),',
             '`handle` (c: interface, b: button, the Continue packet), `close`, `input` (v), `closeEvent` and the other',
             'dialogue-dsl actions. `sweep` is the index of an action whose recorded calls each fail once in turn, or None.',
             '"""', '']
    if plugin:
        lines.append(f"CLASS = '{prefix}{plugin}'")
    lines += [f'NPC = {npc}', '',
              '# Recorded call names the corpus must reach (the generator fails without them).', 'NEEDED_EVENTS = []', '', '',
              'def cases():', '    yield from ()']
    if plugin:
        lines += ['    # A first case to adapt: register, talk to the NPC, then Continue on the chatbox.',
                  "    # talk = [dict(op='register', **{'class': CLASS}), dict(op='openId', key=NPC, npc=0)]",
                  "    # yield dict(name='talk', npcs=[NPC], actions=talk + [dict(op='handle', c=241, b=5)]), 1"]
    return '\n'.join(lines) + '\n'


def test_rs(topic, module, area, calls):
    impls = []
    for call, name in zip(calls, method_names(calls)):
        impls += [f'    fn {name}(&mut self) -> Result<()> {{', f'        self.emit("{rust_str(call)}", vec![])', '    }']
    return '\n'.join([
        f'//! Replays the JVM {topic} corpus (oracle/{topic}) against',
        f'//! `world::content::{area}::{module}` over the dialogue-dsl recording host',
        '//! and the dialogue content kit, and pins the fixture.',
        '#[path = "../../../oracle/dialogue-dsl/native.rs"]',
        'mod native;',
        '#[path = "../../../oracle/content-kit/dialogue.rs"]',
        'mod kit;',
        'use native::Model;',
        'use rs09_server::world::{',
        f'    content::{{self, {area}::{module}}},',
        '    dialogue::{Plugin, Result, Text},',
        '};',
        '',
        'impl content::Services for Model {',
        '    fn settings_name(&mut self) -> Result<Option<Text>> {',
        '        kit::settings_name(self)',
        '    }',
        '}',
        "/// The recording answers of the port's own services: each records its call",
        "/// as the oracle's stand-in does.",
        f'impl {module}::Host for Model {{',
        *impls,
        '}',
        '',
        'fn prototype(class: &str) -> Option<Plugin<Model>> {',
        '    content::dialogue_plugin(class)',
        '}',
        '',
        '#[test]',
        f'fn original_{module}_observations_match() {{',
        '    kit::replay(',
        f'        "{topic}",',
        f'        include_str!("../../../fixtures/{topic}/oracle.jsonl"),',
        f'        include_str!("../../../fixtures/{topic}/provenance.json"),',
        '        prototype,',
        '    );',
        '}',
        ''])


def live_rs(module, area, calls, relative):
    out = [f"//! The live world's answers to `world::content::{area}::{module}::Host`",
           f'//! ({relative}). An answer that needs a system without a port refuses',
           '//! (and is listed in docs/live-world.md); none fabricates a success.',
           'use super::super::host::Live;',
           '#[allow(unused_imports)]',
           f'use crate::world::{{content::{area}::{module}, dialogue::{{Error, Result}}}};',
           '',
           f'impl {module}::Host for Live {{']
    for call, name in zip(calls, method_names(calls)):
        out += [f'    fn {name}(&mut self) -> Result<()> {{',
                f'        Err(Error::Binding("{rust_str(call)} is not answered in the live world yet".into()))',
                '    }']
    return '\n'.join(out + ['}', ''])


def doc_md(topic, relative, found, calls, kind):
    names = ', '.join(f'`{n}`' for n, _ in found)
    services = '\n'.join(f'- `{c}`: TODO (a service, a library call, or not a call)' for c in calls) or '- none found'
    return f'''# {names}

The original {names} (`{relative}`), ported to `world::content` and checked against its own JVM corpus
(`oracle/{topic}`, `fixtures/{topic}`). Laid out by `tools/content_scaffold.py {kind}`; the recipe is
docs/content-porting.md.

## What is ported

TODO: the classes and members, and what the live world does with them (registration, the talk path).

## The corpus

`cargo xtask {topic}-oracle --check` (Python, JDK 11, the pinned reference and ConstLib) compiles the source unchanged
over the dialogue content kit and reproduces `fixtures/{topic}`; `cargo test -p rs09-server --test content_{snake(topic)}`
replays it natively. TODO: the cases and their counts.

## Services

The calls the scaffold found outside the dialogue library, and what became of each:

{services}

## Not covered

TODO: what the corpus and the live binding do not establish.
'''


def rustfmt(written):
    """Formats the Rust files written or registered in, so a fresh scaffold passes `cargo fmt --check`."""
    paths = sorted({str(ROOT / w.split(' ')[0]) for w in written if w.split(' ')[0].endswith('.rs')})
    try:
        subprocess.run(['rustfmt', '--edition', '2024', *paths], cwd=ROOT, check=True)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f'rustfmt did not run ({error}): run cargo fmt')


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('kind', choices=['dialogue', 'generic'])
    parser.add_argument('source', help='the original source, relative to reference/server/Server/src/main (or a full path)')
    parser.add_argument('--topic', help='oracle/fixtures folder name (default: the first class in kebab case)')
    parser.add_argument('--area', help="world/content folder (default: the source's region folder)")
    parser.add_argument('--force', action='store_true', help='overwrite files the scaffold writes')
    parser.add_argument('--no-generate', action='store_true', help='do not run the generator for the empty corpus')
    parser.add_argument('--java-home', help='JDK for the generator (default: RS09_ORACLE_JAVA_HOME, then java on PATH)')
    args = parser.parse_args()
    source = pathlib.Path(args.source)
    relative = source.resolve().relative_to(SERVER.resolve()).as_posix() if source.is_absolute() else source.as_posix()
    text = (SERVER / relative).read_text(encoding='utf-8')
    found = classes(text)
    if not found:
        sys.exit(f'no top-level class in {relative}')
    plugin_classes = [(n, s) for n, s in found if s in ('DialoguePlugin', 'SimpleDialoguePlugin')]
    file_classes = [(n, s) for n, s in found if s in ('DialogueLabeller', 'DialogueFile', 'DialogueBuilderFile')]
    if args.kind == 'dialogue' and not plugin_classes + file_classes:
        sys.exit(f'{relative} declares no dialogue class ({found}); use the generic kind')
    topic = args.topic or kebab(found[0][0])
    module = snake(topic)
    area = args.area or area_of(relative)
    calls, shared = split_shared(outside_calls(text, {n for n, _ in found}))
    ids = ids_of(text)
    package = re.search(r'(?m)^package\s+([\w.]+)', text)
    prefix = package.group(1) + '.' if package else ''
    written = []
    write(f'crates/server/src/world/content/{area}/{module}.rs',
          native_module(relative, topic, plugin_classes, file_classes, ids, calls, shared), args.force, written)
    if not (ROOT / f'crates/server/src/world/content/{area}/mod.rs').exists():
        write(f'crates/server/src/world/content/{area}/mod.rs',
              f'//! Ported content of `{pathlib.PurePosixPath(relative).parent.as_posix()}`.\n// content-scaffold: modules\n',
              False, written)
        insert('crates/server/src/world/content/mod.rs', '// content-scaffold: modules', f'pub mod {area};', written)
    insert(f'crates/server/src/world/content/{area}/mod.rs', '// content-scaffold: modules', f'pub mod {module};', written)
    insert('crates/server/src/world/content/mod.rs', '// content-scaffold: hosts', f'{area}::{module}::Host,', written)
    write(f'docs/{topic}.md', doc_md(topic, relative, found, calls + [f'{c} (content::Services::{s})' for c, s in shared], args.kind),
          args.force, written)
    if args.kind == 'generic':
        rustfmt(written)
        print('Wrote:\n' + '\n'.join('  ' + w for w in written))
        print(f'\nThe generic kind has no shared oracle layer yet: write oracle/{topic}/ from the closest existing corpus '
              '(docs/content-porting.md, "Other content").')
        return
    for name, _ in plugin_classes:
        insert('crates/server/src/world/content/mod.rs', '// content-scaffold: dialogue plugins',
               f'"{prefix}{name}" => {area}::{module}::prototype(),', written)
    write(f'crates/server/src/world/live/ported/{module}.rs', live_rs(module, area, calls, relative), args.force, written)
    insert('crates/server/src/world/live/ported/mod.rs', '// content-scaffold: live modules', f'mod {module};', written)
    write(f'oracle/{topic}/generate.py', generate_py(relative, topic, found), args.force, written)
    write(f'oracle/{topic}/cases.py', cases_py(topic, plugin_classes, ids, prefix), args.force, written)
    write(f'crates/server/tests/content_{module}.rs', test_rs(topic, module, area, calls), args.force, written)
    ci = ROOT / 'oracle/ci.toml'
    if f'"{topic}-oracle"' not in ci.read_text(encoding='utf-8'):
        names = ', '.join(n for n, _ in found)
        with ci.open('a', encoding='utf-8', newline='\n') as f:
            f.write(f'\n[[check]]\nname = "Reproduce original {names} content ({topic})"\nxtask = ["{topic}-oracle", "--check"]\n')
        written.append('oracle/ci.toml (registered)')
    rustfmt(written)
    print('Wrote:\n' + '\n'.join('  ' + w for w in written))
    print('\nOutside calls found (now Host methods; fix their signatures):\n' + ('\n'.join('  ' + c for c in calls) or '  none'))
    if shared:
        print('Already shared services:\n' + '\n'.join(f'  {c} -> content::Services::{s}' for c, s in shared))
    if not args.no_generate:
        command = [sys.executable, str(ROOT / f'oracle/{topic}/generate.py')]
        if args.java_home:
            command += ['--java-home', args.java_home]
        print('\nGenerating the empty corpus: ' + ' '.join(command), flush=True)
        subprocess.run(command, cwd=ROOT, check=True)
    print(f'\nNext: fill oracle/{topic}/cases.py, run `cargo xtask {topic}-oracle` (then --check), port the TODOs in '
          f'world/content/{area}/{module}.rs and run `cargo test -p rs09-server --test content_{module}`.')


if __name__ == '__main__':
    main()
