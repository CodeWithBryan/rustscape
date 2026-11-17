"""Write crates/server/src/world/global_handlers/special_ladders.rs from the unchanged SpecialLadders enum.

Every constant's ladder and destination locations, in declaration order, and its checkAchievement override (a diary task
or the Zogre Flesh Eaters hook). --check compares the committed file instead of writing it. tests/global_handlers.rs and
the oracle corpus (one case per constant) check the table again.
"""
import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'reference/server/Server/src/main/core/game/global/action/SpecialLadders.java'
TARGET = ROOT / 'crates/server/src/world/global_handlers/special_ladders.rs'
DIARIES = ['KARAMJA', 'VARROCK', 'LUMBRIDGE', 'FALADOR', 'FREMENNIK', 'SEERS_VILLAGE']

LOC = r'(?:new Location|Location\.create)\(\s*(-?\d+)\s*,\s*(-?\d+)\s*,\s*(-?\d+)\s*\)'
ENTRY = re.compile(r'^\s+([A-Z_0-9]+)\(\s*' + LOC + r'\s*,\s*' + LOC + r'\s*\)\s*(\{|,|;)', re.M)


def entries(text):
    body = text[text.index('implements LadderAchievementCheck {'):text.index('private static HashMap<Location,Location> destinationMap')]
    out = []
    matches = list(ENTRY.finditer(body))
    for i, m in enumerate(matches):
        name = m.group(1)
        ladder = tuple(int(m.group(k)) for k in (2, 3, 4))
        dest = tuple(int(m.group(k)) for k in (5, 6, 7))
        achievement = 'Achievement::None'
        if m.group(8) == '{':
            end = matches[i + 1].start() if i + 1 < len(matches) else len(body)
            block = body[m.end():end]
            diary = re.search(r'finishTask\(player,\s*DiaryType\.([A-Z_]+),\s*(\d+),\s*(\d+)\)', block)
            if diary:
                achievement = f'Achievement::Diary {{ kind: {DIARIES.index(diary.group(1))}, level: {diary.group(2)}, task: {diary.group(3)} }}'
            elif 'ZogreFleshEatersListeners.ladderMakesSithikTurnIntoOgre(player)' in block:
                achievement = 'Achievement::ZogreSithik'
            else:
                raise RuntimeError(f'unknown override for {name}')
        out.append((name, ladder, dest, achievement))
    declared = re.findall(r'^\s+([A-Z_0-9]+)\(', body, re.M)
    if [e[0] for e in out] != declared:
        raise RuntimeError('an entry did not parse')
    return out


def render(rows):
    lines = [
        '//! `SpecialLadders`\' constants as data: ladder location, destination and the',
        '//! checkAchievement override, in declaration order. Written by',
        '//! oracle/global-handlers/port_special_ladders.py from the unchanged enum;',
        '//! the global-handlers corpus climbs every constant.',
        'use super::climb::{Achievement, SpecialLadder};',
        '',
        '#[rustfmt::skip]',
        f'pub const SPECIAL_LADDERS: [SpecialLadder; {len(rows)}] = [',
    ]
    for name, ladder, dest, achievement in rows:
        lines.append(f'    SpecialLadder {{ name: "{name}", ladder: [{ladder[0]}, {ladder[1]}, {ladder[2]}], '
                     f'destination: [{dest[0]}, {dest[1]}, {dest[2]}], achievement: {achievement} }},')
    lines.append('];')
    return '\n'.join(lines) + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = SOURCE.read_bytes().replace(b'\r', b'').decode()
    data = render(entries(text))
    if args.check:
        if TARGET.read_text(encoding='utf-8') != data:
            sys.exit('special_ladders.rs differs from the enum')
        print(f'{TARGET.relative_to(ROOT)} matches SpecialLadders.')
    else:
        TARGET.write_text(data, encoding='utf-8', newline='\n')
        print(f'Wrote {TARGET.relative_to(ROOT)}.')


if __name__ == '__main__':
    main()
