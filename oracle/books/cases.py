"""The books corpus: (case, sweep) pairs; `sweep` is the index of an operation whose every recorded call is made to
fail once in turn by generate.py's second pass, or None."""

IFACE = 'content.global.handlers.iface.BookInterface'
BOOKS = {
    'GoblinBook': (10999, 26, 6),
    'GrimDiary': (11780, 26, 8),
    'SecurityBookPlugin': (9003, 27, 4),
    'StrongholdNotes': (9004, 27, 8),
}
ARROWS = {26: (61, 63), 27: (1, 3), 49: (51, 53)}
INDEX = [104, 106, 108, 110, 112, 114, 116, 118, 120, 122, 124, 126, 128, 134, 136, 138, 140, 142, 144, 146]
NEEDED_EVENTS = ['on', 'onInterface', 'closeInterface', 'openInterface', 'setAttribute', 'getAttribute', 'sendString', 'interfaceConfig',
                 'modelOnInterface', 'angleOnInterface', 'itemOnInterface', 'storeBookInHouse']


def case(name, ops, world=None, answers=None, group='books'):
    c = {'name': name, 'group': group, 'ops': ops}
    if world:
        c['world'] = world
    if answers:
        c['answers'] = answers
    return c


def define(*names):
    return [{'op': 'define', 'class': IFACE if n == 'BookInterface' else 'content.global.handlers.item.book.' + n} for n in names]


def cases():
    yield case('every listener registers', define('BookInterface', 'GoblinBook', 'GrimDiary', 'SecurityBookPlugin', 'StrongholdNotes'),
               group='registration'), None
    for name, (item, component, sets) in BOOKS.items():
        left, right = ARROWS[component]
        ops = define('BookInterface', name) + [{'op': 'read', 'item': item}]
        forward = [{'op': 'button', 'component': component, 'button': right} for _ in range(sets)]
        back = [{'op': 'button', 'component': component, 'button': left} for _ in range(sets + 1)]
        other = [{'op': 'button', 'component': component, 'button': b} for b in (100, 70, 159, 0)]
        yield case(f'{name}: every page forward and back', ops + forward + back + other, group=name), 2
        yield case(f'{name}: a second read', ops + forward[:2] + [{'op': 'read', 'item': item}] + forward[:1], group=name), 4
        yield case(f'{name}: not stored in the house', ops + forward[:1], answers={'storeBookInHouse': False}, group=name), None
        yield case(f'{name}: a page turn failing', ops + forward[:1] + [{'op': 'button', 'component': component, 'button': right}],
                   group=name), 4
        yield case(f'{name}: the other components\' buttons', ops + [
            {'op': 'button', 'component': c, 'button': b} for c in (26, 27, 49) for b in ARROWS[c] + (7,)
        ], group=name), None
    yield case('names of another world', define('BookInterface', 'GoblinBook', 'SecurityBookPlugin') + [
        {'op': 'read', 'item': 10999}, {'op': 'button', 'component': 26, 'button': 63}, {'op': 'button', 'component': 26, 'button': 63},
        {'op': 'button', 'component': 26, 'button': 63}, {'op': 'button', 'component': 26, 'button': 63},
        {'op': 'button', 'component': 26, 'button': 63},
        {'op': 'read', 'item': 9003}, {'op': 'button', 'component': 27, 'button': 3},
    ], world={'settingsName': 'TestScape', 'serverName': 'Elsewhere'}, group='names'), None
    rules = define('BookInterface') + [{'op': 'rules'}]
    yield case('the rules book', rules + [
        {'op': 'button', 'component': 27, 'button': 3} for _ in range(9)
    ] + [{'op': 'button', 'component': 27, 'button': 1} for _ in range(10)], group='rules'), 1
    for b in INDEX:
        yield case(f'the rules index button {b}', rules + [
            {'op': 'button', 'component': 27, 'button': b},
            {'op': 'button', 'component': 27, 'button': b},
            {'op': 'button', 'component': 27, 'button': 159},
            {'op': 'button', 'component': 27, 'button': 159},
        ], group='rules'), 2 if b in (104, 146) else None
    yield case('the rules book\'s other buttons', rules + [
        {'op': 'button', 'component': 27, 'button': b} for b in (100, 102, 130, 148, 158, 5, 3, 100, 160, 1)
    ], group='rules'), None
    yield case('the rules names of another world', rules, world={'settingsName': 'Gielinor Revived'}, group='rules'), None
    yield case('a book without a callback', define('BookInterface', 'GrimDiary') + [
        {'op': 'read', 'item': 11780},
        {'op': 'attr', 'key': 'bookInterfaceCallback', 'value': None},
        {'op': 'button', 'component': 26, 'button': 63},
        {'op': 'button', 'component': 49, 'button': 51},
    ], group='attributes'), 4
    yield case('mistyped book attributes', define('BookInterface', 'StrongholdNotes') + [
        {'op': 'read', 'item': 9004},
        {'op': 'attr', 'key': 'bookInterfaceCurrentPage', 'value': 'two'},
        {'op': 'button', 'component': 27, 'button': 3},
        {'op': 'attr', 'key': 'bookInterfaceCurrentPage', 'value': 3},
        {'op': 'button', 'component': 27, 'button': 3},
        {'op': 'attr', 'key': 'bookInterfaceCallback', 'value': 5},
        {'op': 'button', 'component': 27, 'button': 3},
    ], group='attributes'), None
    yield case('a page set far outside the book', define('BookInterface', 'SecurityBookPlugin') + [
        {'op': 'read', 'item': 9003},
        {'op': 'attr', 'key': 'bookInterfaceCurrentPage', 'value': 40},
        {'op': 'button', 'component': 27, 'button': 100},
        {'op': 'attr', 'key': 'bookInterfaceCurrentPage', 'value': -3},
        {'op': 'button', 'component': 27, 'button': 3},
    ], group='attributes'), None
    yield case('models and items on pages', define('BookInterface', 'SecurityBookPlugin') + [
        {'op': 'read', 'item': 9003},
        {'op': 'model', 'pageSet': 0, 'model': 2400, 'component': 27, 'enable': 9, 'draw': 10, 'zoom': 600, 'pitch': 100, 'yaw': 30},
        {'op': 'model', 'pageSet': 1, 'model': 2401, 'component': 27, 'enable': 11, 'draw': 12, 'zoom': 600, 'pitch': 100, 'yaw': 30},
        {'op': 'itemOnPage', 'pageSet': 0, 'itemId': 995, 'component': 27, 'enable': 13, 'draw': 14, 'zoom': 1, 'pitch': 2, 'yaw': 3},
        {'op': 'itemOnPage', 'pageSet': 2, 'itemId': 996, 'component': 27, 'enable': 13, 'draw': 14, 'zoom': 1, 'pitch': 2, 'yaw': 3},
        {'op': 'clearModels', 'component': 27},
        {'op': 'clearModels', 'component': 26},
        {'op': 'isLastPage', 'page': 3, 'total': 4},
        {'op': 'isLastPage', 'page': 2, 'total': 4},
        {'op': 'isLastPage', 'page': -1, 'total': 0},
        {'op': 'attr', 'key': 'bookInterfaceCurrentPage', 'value': 'x'},
        {'op': 'model', 'pageSet': 0, 'model': 1, 'component': 27, 'enable': 9, 'draw': 10, 'zoom': 1, 'pitch': 1, 'yaw': 1},
    ], group='pictures'), 3


def groups(cases):
    found = {}
    for c in cases:
        found[c['group']] = found.get(c['group'], 0) + 1
    return dict(sorted(found.items()))
