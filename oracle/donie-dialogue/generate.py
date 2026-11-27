"""Run the original DonieDialogue, DonieDialogueFile, DonieDialogueShoelaceFile (content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt) on JDK 11 over the dialogue content kit
(oracle/content-kit/dialogue.py) and record what it does. Laid out by tools/content_scaffold.py.

The source is compiled unchanged on top of the dialogue-dsl build (its unchanged classes, inserted original text and
recording stand-ins) with the kit's stand-ins. Java/Kotlin are test oracles only; --check never rewrites a fixture.
"""
import importlib.util
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
RUN = 'runs/donie-dialogue-oracle'  # where the kit builds (tools/run_oracles.py reads this name)


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


kit = load('content_kit_dialogue', ROOT / 'oracle/content-kit/dialogue.py')
cases = load('donie_dialogue_cases', HERE / 'cases.py')

SOURCES = ['content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt']
SCOPE = (
    'content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt compiled unchanged on JDK 11 with Kotlin 1.8.20 and ConstLib 1.4.28 over the dialogue content kit: '
    'the dialogue-dsl build (its unchanged dialogue core and DSLs, inserted original text and recording stand-ins) '
    'with the kit stand-ins. Registration is ClassScanner\'s DIALOGUE branch (the no-argument constructor, init()); '
    'talking is DialogueInterpreter.open(id, npc). Covered: registration of id 2238, the greeting, the three '
    'options and each branch to its end (the weather line with the kit world name and another server name, the '
    'shoelace file), a female player, Continue after an end, closing mid-way, an unregistered key, a registration alone, '
    'opening without an NPC (the null npc field of the plugin), an option button out of range and no settings (the '
    'core fails at @servername before the "2009Scape" default of Donie reaches a line). Failure sweeps fail each '
    'recorded call of one action per case once in turn (provenance coverage.failedEvents counts them by call); most '
    'are calls of the dialogue core and DSLs on the path of Donie, her own being GameWorld.settings. Not covered: '
    'packet bytes, real interfaces or definitions, NPCTalkListener and the live session (the donie scenario of '
    'live_world).'
)

if __name__ == '__main__':
    kit.main('donie-dialogue', SOURCES, cases, SCOPE, __doc__)
