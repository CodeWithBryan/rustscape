"""Write crates/server/src/world/player_managers/diary_data.rs from DiaryType.

The table is the one the unchanged DiaryType reports in the player-managers
corpus (its `diaryTable` case), so the data file is a mechanical copy;
tests/player_managers.rs compares it with that case again. --check only
compares.
"""
import argparse
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
TARGET = ROOT / 'crates/server/src/world/player_managers/diary_data.rs'


def rust(text):
    out = ''
    for ch in text:
        if ch in '\\"':
            out += '\\' + ch
        elif 32 <= ord(ch) < 127:
            out += ch
        else:
            out += '\\u{%x}' % ord(ch)
    return '"' + out + '"'


def render(table):
    lines = [
        '//! DiaryType\'s six constants as data: name, tab child, level names, task',
        '//! lines, reward items (id, amount), start info and reward NPCs, in',
        '//! declaration order. Written by oracle/player-managers/port_diary_data.py',
        '//! from the unchanged enum; tests/player_managers.rs compares it again.',
        '',
        '/// One DiaryType constant.',
        'pub struct DiaryType {',
        '    pub constant: &\'static str,',
        '    pub name: &\'static str,',
        '    pub child: i32,',
        '    pub level_names: &\'static [&\'static str],',
        '    pub achievements: &\'static [&\'static [&\'static str]],',
        '    pub rewards: &\'static [&\'static [(i32, i32)]],',
        '    pub info: &\'static str,',
        '    pub npcs: &\'static [i32],',
        '}',
        '',
        '#[rustfmt::skip]',
        f'pub const TYPES: [DiaryType; {len(table)}] = [',
    ]
    for constant, name, child, levels, achievements, rewards, info, npcs in table:
        lines.append('    DiaryType {')
        lines.append(f'        constant: {rust(constant)},')
        lines.append(f'        name: {rust(name)},')
        lines.append(f'        child: {child},')
        lines.append('        level_names: &[' + ', '.join(rust(n) for n in levels) + '],')
        lines.append('        achievements: &[')
        for level in achievements:
            lines.append('            &[')
            for task in level:
                lines.append(f'                {rust(task)},')
            lines.append('            ],')
        lines.append('        ],')
        lines.append('        rewards: &[' + ', '.join('&[' + ', '.join(f'({i}, {a})' for i, a in level) + ']' for level in rewards) + '],')
        lines.append(f'        info: {rust(info)},')
        lines.append('        npcs: &[' + ', '.join(str(n) for n in npcs) + '],')
        lines.append('    },')
    lines.append('];')
    return '\n'.join(lines) + '\n'


def main(check):
    for line in (ROOT / 'fixtures/player-managers/oracle.jsonl').read_text(encoding='utf-8').splitlines():
        row = json.loads(line)
        if row['actions'][0]['op'] == 'diaryTable':
            text = render(row['expected']['results'][0]['value'])
            break
    else:
        raise RuntimeError('no diaryTable case')
    if check:
        if TARGET.read_text(encoding='utf-8') != text:
            raise RuntimeError('diary_data.rs differs from the corpus table')
        print('diary_data.rs matches.')
    else:
        TARGET.write_text(text, encoding='utf-8', newline='\n')
        print(f'Wrote {TARGET}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    main(parser.parse_args().check)
