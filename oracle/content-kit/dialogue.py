"""Shared generator for dialogue content oracles (docs/content-porting.md).

A content dialogue (a DialoguePlugin, DialogueLabeller or DialogueBuilderFile subclass) runs on the dialogue core and
DSLs the dialogue-dsl oracle already compiles unchanged over recording stand-ins. This module reuses that build
(oracle/dialogue-dsl/generate.py: its unchanged list, markers, stand-ins, adapters, scripts and harness) and adds:

- the content sources, compiled unchanged;
- the kit's replacement stand-ins (`stand-ins/`), which take the place of a dialogue-dsl stand-in of the same path
  (GameWorld becomes a Kotlin object, so content's `GameWorld.settings?.name` compiles as it does against the
  original);
- harness actions spliced into the dialogue-dsl Oracle.act (DialogueOps.java): `register` (ClassScanner's DIALOGUE
  branch: the no-argument constructor, then init()) and `openId` (DialogueInterpreter.open(id, npc), the call
  NPCTalkListener makes).

A topic's generate.py names its sources and cases module and calls `main`. The dialogue-dsl fixtures are not
touched: this build writes to runs/<topic>-oracle. Java/Kotlin are test oracles only; --check never rewrites a fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
KIT = pathlib.Path(__file__).resolve().parent
SERVER = ROOT / 'reference/server/Server/src/main'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# The dialogue-dsl generator; its module globals are pointed at the topic before the build.
dsl = load('dialogue_dsl_generate', ROOT / 'oracle/dialogue-dsl/generate.py')
c = dsl.c

OPS_MARK = '   default:throw new AssertionError(a);'
HELPERS_MARK = ' static Object comp(Component c)'


def stand_ins():
    folder = KIT / 'stand-ins'
    return {p.relative_to(folder).as_posix(): p for p in sorted(folder.rglob('*')) if p.is_file()}


def kit_bundles(original_bundles):
    """The dialogue-dsl bundles with the kit's stand-ins in place of their namesakes and the kit actions spliced in."""
    replacements = {name: c.read(path) for name, path in stand_ins().items()}
    stems = {name.rsplit('.', 1)[0] for name in replacements}

    def bundles(inserted):
        out = []
        for name, text in original_bundles(inserted):
            if name.rsplit('.', 1)[0] in stems:
                continue
            if name == 'oracle/Oracle.java':
                actions, helpers = c.read(KIT / 'DialogueOps.java').split('//// actions\n', 1)[1].split('//// helpers\n', 1)
                if text.count(OPS_MARK) != 1 or text.count(HELPERS_MARK) != 1:
                    raise RuntimeError('the dialogue-dsl harness changed: the kit actions no longer splice in')
                text = text.replace(OPS_MARK, actions + OPS_MARK).replace(HELPERS_MARK, helpers + HELPERS_MARK)
            out.append((name, text))
        return out + sorted(replacements.items())
    return bundles


def main(topic, unchanged, cases, scope, doc=None):
    """Build, run and compare (or write) the topic's corpus.

    topic      the folder name under oracle/ and fixtures/ (`runs/<topic>-oracle` is the build directory)
    unchanged  the content sources, relative to the pinned server's src/main, compiled unchanged
    cases      the topic's cases module: `cases()` yields (case, sweep) as the dialogue-dsl corpus does, sweep being
               the index of the action whose recorded calls each fail once in turn (None for no sweep)
    scope      what the corpus establishes and what it does not (provenance `scope`)
    """
    parser = argparse.ArgumentParser(description=doc)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    fixtures = ROOT / 'fixtures' / topic
    dsl.RUN = ROOT / 'runs' / f'{topic}-oracle'
    dsl.UNCHANGED = list(dsl.UNCHANGED) + [name for name in unchanged if name not in dsl.UNCHANGED]
    dsl.bundles = kit_bundles(dsl.bundles)
    # The content sources name ConstLib constants (NPCs.DONIE_2238): check their values against their names too.
    check_constants = dsl.constants_match_names
    dsl.constants_match_names = lambda classpath, texts: check_constants(classpath, texts + [c.read(SERVER / n) for n in unchanged])
    lock, classpath, checked = dsl.build()
    planned = list(cases.cases())
    rows = dsl.execute(classpath, [case for case, _ in planned]) if planned else []
    # A failure case is the designed case cut after its swept action, with the at-th recorded call of that action
    # failing. Two designed cases with the same world and prefix give the same failure cases: keep the first only
    # (`failedEvents` then counts distinct failures).
    failing, seen, failed_events = [], set(), {}
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at, event in enumerate(row['expected'][sweep]['events'], 1):
            actions = [dict(a, failAt=at) if k == sweep else a for k, a in enumerate(case['actions'][:sweep + 1])]
            key = json.dumps(dict(case, name=None, actions=actions), sort_keys=True)
            if key in seen:
                continue
            seen.add(key)
            failed_events[event[0]] = failed_events.get(event[0], 0) + 1
            failing.append(dict(case, name=f"{case['name']} fail {at}", actions=actions))
    if failing:
        rows += dsl.execute(classpath, failing)
    mistakes = {}
    for number, row in enumerate(rows):
        for action, step in zip(row['actions'], row['expected']):
            error = step['error']
            if error not in dsl.EXPECTED_ERRORS or (error == 'Binding' and 'failAt' not in action):
                mistakes.setdefault(error, (number, row.get('name'), action))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for action, step in zip(row['actions'], row['expected']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if step['error']:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    # A failure case repeats its designed case up to the swept action: keep only that action's snapshot.
    for row in rows[len(planned):]:
        row['expected'] = row['expected'][-1:]
        row['onlyLast'] = True
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    for path in [ROOT / 'oracle' / topic / 'generate.py', pathlib.Path(cases.__file__), KIT / 'dialogue.py', KIT / 'DialogueOps.java', ROOT / 'oracle/dialogue-dsl/generate.py',
                 ROOT / 'oracle/login/generate.py', *stand_ins().values()]:
        c.read(path)
    for needed in getattr(cases, 'NEEDED_EVENTS', []):
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), designedCases=len(planned),
                    failureCases=len(failing), failedEvents=dict(sorted(failed_events.items())))
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', content=list(unchanged),
        unchanged=dsl.UNCHANGED, constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), scope=scope)
    summary = (f'{len(rows)} original {topic} cases ({len(planned)} designed, {len(failing)} failure cases, '
               f'{coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls)')
    if args.check:
        # An empty or unfinished corpus is not a port: refuse it here and in the replay (oracle/content-kit/dialogue.rs).
        if not planned:
            raise RuntimeError(f'{topic} has no designed cases: write oracle/{topic}/cases.py first')
        if 'TODO' in scope:
            raise RuntimeError(f'{topic}: the scope still says TODO: write what the cases cover and what they do not')
        if not (fixtures / 'oracle.jsonl').exists():
            raise RuntimeError(f'fixtures/{topic} has no corpus yet: run the generator without --check first')
        if (fixtures / 'oracle.jsonl').read_bytes() != data or json.loads((fixtures / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError(f'{topic} fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        fixtures.mkdir(parents=True, exist_ok=True)
        (fixtures / 'oracle.jsonl').write_bytes(data)
        (fixtures / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8',
                                                     newline='\n')
        print(f'Generated {summary}.')
        if not planned or 'TODO' in scope:
            print(f'Not a port yet: --check and the replay refuse it until cases.py has designed cases and SCOPE no TODO.')
