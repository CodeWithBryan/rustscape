"""Port the five global books' page tables to crates/server/src/world/books/contents.rs.

Reads the unchanged GeneralRuleBook, GoblinBook, GrimDiary, SecurityBookPlugin and StrongholdNotes and writes their
TITLE and CONTENTS (PageSet, Page, BookLine(message, child)) as Rust data: each message is a list of segments, string
literals (BLUE/RED inlined) and the two world names the books concatenate (`GameWorld.settings!!.name` and
`ServerConstants.SERVER_NAME`), which a running world supplies when the book's class initialises. --check compares
the committed file instead of writing it. The table is laid out by rustfmt (stdin to stdout). The books-oracle replay is the behavioural check of the result.
"""
import argparse
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'reference/server/Server/src/main/content/global/handlers/item/book'
TARGET = ROOT / 'crates/server/src/world/books/contents.rs'
BOOKS = [('GeneralRuleBook', 'GENERAL_RULES'), ('GoblinBook', 'GOBLIN'), ('GrimDiary', 'GRIM_DIARY'),
         ('SecurityBookPlugin', 'SECURITY'), ('StrongholdNotes', 'STRONGHOLD_NOTES')]
NAMES = {'GameWorld.settings!!.name': 'Seg::Settings', 'SERVER_NAME': 'Seg::Settings', 'ServerConstants.SERVER_NAME': 'Seg::Server'}


def tokens(text):
    pattern = re.compile(r'\s+|//[^\n]*|"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z0-9_.!]*|-?\d+|[(),+]')
    position = 0
    while position < len(text):
        m = pattern.match(text, position)
        if not m:
            raise RuntimeError(f'unexpected text {text[position:position + 40]!r}')
        position = m.end()
        token = m.group(0)
        if token.strip() and not token.startswith('//'):
            yield token


class Parser:
    def __init__(self, text, constants):
        self.items = list(tokens(text))
        self.at = 0
        self.constants = constants

    def peek(self):
        return self.items[self.at] if self.at < len(self.items) else None

    def take(self, expected=None):
        token = self.items[self.at]
        if expected is not None and token != expected:
            raise RuntimeError(f'expected {expected}, found {token}')
        self.at += 1
        return token

    def call(self, name, item):
        self.take(name)
        self.take('(')
        found = []
        while self.peek() != ')':
            found.append(item())
            if self.peek() == ',':
                self.take(',')
        self.take(')')
        return found

    def term(self):
        token = self.take()
        if token.startswith('"'):
            return ('lit', token[1:-1])
        if token in NAMES:
            return ('name', NAMES[token])
        if token in self.constants:
            value = self.constants[token]
            if isinstance(value, str):
                return ('lit', value)
        raise RuntimeError(f'unknown term {token}')

    def expr(self):
        parts = [self.term()]
        while self.peek() == '+':
            self.take('+')
            parts.append(self.term())
        return parts

    def child(self):
        token = self.take()
        if re.fullmatch(r'-?\d+', token):
            return int(token)
        value = self.constants[token]
        if not isinstance(value, int):
            raise RuntimeError(f'child {token}')
        return value

    def line(self):
        self.take('BookLine')
        self.take('(')
        message = self.expr()
        self.take(',')
        child = self.child()
        if self.peek() == ',':
            self.take(',')
        self.take(')')
        return message, child


def constants(text):
    found = {}
    for name, value in re.findall(r'private const val (\w+) = ("[^"]*"|\d+)', text):
        found[name] = value[1:-1] if value.startswith('"') else int(value)
    return found


def balanced(text, start):
    depth, at, quoted = 0, start, False
    while True:
        ch = text[at]
        if quoted:
            quoted = ch != '"'
        elif ch == '"':
            quoted = True
        elif ch == '(':
            depth += 1
        elif ch == ')':
            depth -= 1
            if depth == 0:
                return text[start:at + 1]
        at += 1


def segments(parts):
    merged = []
    for kind, value in parts:
        if kind == 'lit' and merged and merged[-1][0] == 'lit':
            merged[-1] = ('lit', merged[-1][1] + value)
        else:
            merged.append((kind, value))
    return ', '.join(f'Seg::T("{v}")' if k == 'lit' else v for k, v in merged)


def title(name, text):
    if name == 'GeneralRuleBook':
        if 'private val TITLE = "$SERVER_NAME Rules"' not in text:
            raise RuntimeError('GeneralRuleBook TITLE changed')
        return 'Seg::Settings, Seg::T(" Rules")'
    m = re.search(r'private (?:const )?val TITLE = (.+)', text)
    parser = Parser(m.group(1), constants(text))
    return segments(parser.expr())


def render():
    out = ['//! The five global books\' titles and pages, ported by oracle/books/port_contents.py from the unchanged',
           '//! GeneralRuleBook, GoblinBook, GrimDiary, SecurityBookPlugin and StrongholdNotes. Do not edit by hand.',
           'use super::{Line, Seg};', '']
    for name, constant in BOOKS:
        text = (SOURCE / f'{name}.kt').read_text(encoding='utf-8').replace('\r', '')
        start = text.index('CONTENTS = arrayOf(') + len('CONTENTS = ')
        parser = Parser(balanced(text, start), constants(text))
        sets = parser.call('arrayOf', lambda: parser.call('PageSet', lambda: parser.call('Page', parser.line)))
        out.append(f'/// {name}.TITLE.')
        out.append(f'pub const {constant}_TITLE: &[Seg] = &[{title(name, text)}];')
        out.append(f'/// {name}.CONTENTS: page sets, pages, lines (message, child).')
        out.append(f'pub const {constant}: &[&[&[Line]]] = &[')
        for page_set in sets:
            out.append('    &[')
            for page in page_set:
                out.append('        &[')
                for message, child in page:
                    out.append(f'            Line(&[{segments(message)}], {child}),')
                out.append('        ],')
            out.append('    ],')
        out.append('];')
        out.append('')
    # The committed file is rustfmt's layout of the table (cargo fmt --check keeps it so).
    result = subprocess.run(['rustfmt', '--edition', '2024'], input='\n'.join(out).encode(), capture_output=True, check=True)
    return result.stdout.decode('utf-8').replace('\r\n', '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = render()
    if args.check:
        if TARGET.read_text(encoding='utf-8').replace('\r', '') != text:
            raise SystemExit('contents.rs differs from the ported book tables')
        print('book tables reproduced')
    else:
        TARGET.parent.mkdir(parents=True, exist_ok=True)
        TARGET.write_text(text, encoding='utf-8', newline='\n')
        print('book tables written')
