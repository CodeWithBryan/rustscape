"""The Grand Exchange corpus: authored item tables, players and action lists.

Every case runs on its own database file. Item definitions are authored
inputs (the real item cache is not used). A case names its item table; the
tables are stored once in fixtures/grand-exchange/inputs.json.
"""
INT_MAX = (1 << 31) - 1


def item(name, value=1, **kw):
    spec = dict(name=name, value=value, tradeable=True, examine=f'{name} examine.')
    spec.update(kw)
    return spec


BASIC = {
    995: item('Coins', stackable=True),
    1511: item('Logs', 4, noteId=1512),
    1512: item('Logs', 4, unnoted=False, noteId=1511),
    1513: item('Magic logs', 320, noteId=1514),
    1514: item('Magic logs', 320, unnoted=False, noteId=1513),
    4151: item('Abyssal whip', 120001, high_alchemy=72000, low_alchemy=48000, noteId=4152),
    4152: item('Abyssal whip', 120001, unnoted=False, noteId=4151),
    379: item('Lobster', 150, noteId=380),
    380: item('Lobster', 150, unnoted=False, noteId=379),
    554: item('Fire rune', 4, stackable=True),
    590: item('Tinderbox', 1),
    1925: item('Bucket', 2, noteId=1926),
    1926: item('Bucket', 2, unnoted=False, noteId=1925),
    2572: item('Ring of wealth', 1000, noteId=2573),
    2573: item('Ring of wealth', 1000, unnoted=False, noteId=2572),
    14638: item('Ring of wealth (i)', 1000),
    7462: item('Barrows gloves', 130000, tradeable=False),
    4045: item('Explosive potion', 50, options=['Drink', None, None, None, 'Destroy']),
    11238: item('Baby impling jar', 50, options=['Loot', None, None, None, 'Destroy']),
    13000: item('Gold statue', 2000000000),
    1: item('Toolkit', 0),
    2000: item('Mystery note', 10, unnoted=False, noteId=2001),
    12345: dict(name='Untradeable thing', value=7, examine='Not for sale.'),
    7936: item('Pure essence', 1, noteId=7937),
    7937: item('Pure essence', 1, unnoted=False, noteId=7936),
}
for n in range(30):
    BASIC[3000 + n] = item(f'Filler {n}', 10 + n * 7)

SET_ITEMS = dict(BASIC)
SET_ITEMS.update({
    11814: item('Bronze set (lg)', 600),
    1155: item('Bronze full helm', 44), 1117: item('Bronze platebody', 160), 1075: item('Bronze platelegs', 80), 1189: item('Bronze kiteshield', 68),
    11842: item('Dragon chain set', 1000000),
    1149: item('Dragon med helm', 100000), 3140: item('Dragon chainbody', 250000), 4087: item('Dragon platelegs', 270000),
    11967: item('Dwarf cannon set', 750000),
    6: item('Cannon base', 187500), 8: item('Cannon stand', 187500), 10: item('Cannon barrels', 187500), 12: item('Cannon furnace', 187500),
})

ITEMS = {'basic': BASIC, 'sets': SET_ITEMS}


class Corpus:
    def __init__(self):
        self.cases = []
        self.items = {name: {str(k): v for k, v in table.items()} for name, table in ITEMS.items()}

    def case(self, actions, players=None, table='basic', **kw):
        row = dict(name=f'c{len(self.cases):04d}', itemsTable=table, players=players or [P('alice', 11, RICH)], actions=actions)
        row.update(kw)
        self.cases.append(row)


def P(name, uid, inv=(), **kw):
    spec = dict(name=name, uid=uid, inv=[list(r) for r in inv])
    spec.update(kw)
    return spec


def a(op, **kw):
    return dict(op=op, **kw)


def btn(p, iface, opcode, button, slot=0, item=-1):
    return a('button', p=p, iface=iface, opcode=opcode, button=button, slot=slot, item=item)


BUY_SLOT = [30, 46, 62, 81, 100, 119]
SELL_SLOT = [31, 47, 63, 82, 101, 120]
VIEW_SLOT = [18, 34, 50, 69, 88, 107]

RICH = [(0, 995, 10000000), (1, 1511, 100), (2, 1512, 500), (3, 4151, 1), (4, 379, 20), (5, 554, 5000), (6, 2572, 1), (7, 7462, 1),
        (8, 4045, 1), (9, 11238, 1), (10, 1513, 10), (11, 1514, 40)]


def open_ge(p=0):
    return a('openFor', p=p)


def buy(p, slot, item, amount=None, price=None, confirm=True):
    """The buy flow: slot's buy button, the search's chosen item, amount/price by input, confirm."""
    steps = [open_ge(p), btn(p, 105, 155, BUY_SLOT[slot]), a('setOfferItem', p=p, item=item)]
    if amount is not None:
        steps += [btn(p, 105, 155, 170), a('input', p=p, text=str(amount))]
    if price is not None:
        steps += [btn(p, 105, 155, 185), a('input', p=p, int=price)]
    if confirm:
        steps.append(btn(p, 105, 155, 190))
    return steps


def sell(p, slot, inv_slot, amount=None, price=None, confirm=True):
    steps = [open_ge(p), btn(p, 105, 155, SELL_SLOT[slot]), btn(p, 107, 155, 18, inv_slot)]
    if amount is not None:
        steps += [btn(p, 105, 155, 170), a('input', p=p, text=str(amount))]
    if price is not None:
        steps += [btn(p, 105, 155, 185), a('input', p=p, int=price)]
    if confirm:
        steps.append(btn(p, 105, 155, 190))
    return steps


def worker():
    return a('worker')


def view(p, slot, op=155):
    return btn(p, 105, op, VIEW_SLOT[slot])


def collect(p, which, op=196):
    return btn(p, 105, op, 209 if which == 0 else 211)


def setup_cases(k):
    k.case([])
    k.case([], startup='init')
    k.case([a('canTrade', item=1511), a('value', item=1511), a('price', item=1511, bot=True), worker()], startup='none')
    k.case([a('boot'), worker(), a('canTrade', item=1511)], startup='init')
    k.case([a('boot'), a('boot'), worker()])
    # Existing tables: nothing is created and the price index is not filled.
    full = ['CREATE TABLE player_offers(uid INTEGER PRIMARY KEY ASC,player_uid INTEGER,item_id INTEGER,amount_total INTEGER,amount_complete INTEGER,'
            'offered_value INTEGER,time_stamp INTEGER,offer_state INTEGER,is_sale INTEGER,withdraw_items STRING,slot_index INTEGER,total_coin_xc INTEGER)',
            'CREATE TABLE bot_offers(item_id INTEGER,amount INTEGER)',
            'CREATE TABLE price_index(item_id INTEGER,value INTEGER,total_value INTEGER,unique_trades INTEGER,last_update INTEGER)']
    k.case([a('canTrade', item=1511), a('price', item=1511)], seed=full)
    k.case([a('canTrade', item=1511)], seed=full[:1])
    k.case([a('canTrade', item=1511)], seed=full[1:2])
    k.case([a('canTrade', item=1511), a('value', item=1511)], seed=full[2:] + ['INSERT INTO price_index VALUES (1511, 77, 770, 10, 5)'])
    k.case([], table='sets')
    k.case([], players=[P('alice', 11, RICH, login=False)])
    k.case([], players=[P('alice', 11, RICH), P('bob', 22, RICH, online=False)])
    k.case([], clock=0, step=0)


def price_cases(k):
    for item in [1511, 1512, 4151, 4152, 1513, 7462, 4045, 11238, 12345, 2000, 1, 13000, 999999, -5, 0, 7936]:
        k.case([a('canTrade', item=item), a('value', item=item), a('price', item=item, bot=False), a('price', item=item, bot=True)])
    k.case([a('ban', item=1511), a('canTrade', item=1511), a('value', item=1511), a('allow', item=1511), a('canTrade', item=1511),
            a('allow', item=1511), a('value', item=1511), a('ban', item=99), a('allow', item=99), a('value', item=99)])
    for amount, price in [(1, 10), (5, 999999), (5, 1000000), (5, 100000), (5, 25000), (5, 24999), (10001, 5), (9999, 2), (1000, 100000),
                          (501, 1000000), (2501, 30000), (INT_MAX, 2), (-3, 10), (0, 0),
                          (1001, 100000), (1001, 99999), (2501, 25000), (2501, 24999), (501, 999999), (10001, 24999)]:
        k.case([a('addTrade', item=1511, amount=amount, price=price), a('value', item=1511), a('price', item=1511, bot=True)])
    k.case([a('addTrade', item=99, amount=1, price=1)])
    k.case([a('sql', statements=['UPDATE price_index SET unique_trades = -5 WHERE item_id = 1511']), a('addTrade', item=1511, amount=5, price=10)])
    k.case([a('sql', statements=['UPDATE price_index SET unique_trades = 9999, total_value = 99990 WHERE item_id = 1511']),
            a('addTrade', item=1511, amount=2, price=10), a('value', item=1511)])
    k.case([a('addTrade', item=1511, amount=3, price=7) for _ in range(5)], step=0)


def offer_cases(k):
    rich = [P('alice', 11, RICH)]
    # Opening: ironman, bank PIN, plain.
    k.case([open_ge()])
    k.case([open_ge()], players=[P('alice', 11, RICH, ironman='STANDARD')])
    k.case([open_ge()], players=[P('alice', 11, RICH, ironman='ULTIMATE')])
    k.case([open_ge(), a('pin', p=0, value=True), open_ge()], players=[P('alice', 11, RICH, pin=False)])
    k.case([open_ge(), a('close', p=0), a('closeChatbox', p=0)])
    # Buying: guide price, amounts, prices, inputs, confirmation results.
    k.case(buy(0, 0, 1511))
    k.case(buy(0, 0, 1511, 25, 7))
    k.case(buy(0, 1, 4151, 2, 150000))
    k.case(buy(0, 0, 1511, '3k', 1))
    k.case(buy(0, 0, 1511, '1m', 1))
    k.case(buy(0, 0, 1511, '2K', 3))
    k.case(buy(0, 0, 1511, 'abc', 3))
    k.case(buy(0, 0, 1511, '99999999999', 3))
    k.case(buy(0, 0, 1511, 5, 0))
    k.case(buy(0, 0, 13000, 2))
    k.case(buy(0, 0, 1511, 10, 2000000))
    k.case(buy(0, 0, 1511, 3000000, 4))
    k.case(buy(0, 0, 7462))
    k.case(buy(0, 0, 4045))
    k.case(buy(0, 0, 11238))
    k.case(buy(0, 0, 12345))
    k.case(buy(0, 0, 1512))
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[0]), a('setOfferItem', p=0, item=1511)] +
           [btn(0, 105, 155, b) for b in [157, 159, 162, 164, 166, 168, 171, 173, 177, 180, 183, 157, 157, 157]] + [btn(0, 105, 155, 190)])
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[2]), a('setOfferItem', p=0, item=4151), btn(0, 105, 155, 157), btn(0, 105, 155, 157),
            btn(0, 105, 155, 190)])
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[0]), a('setOfferItem', p=0, item=1511), btn(0, 105, 155, 171), btn(0, 105, 155, 171),
            btn(0, 105, 155, 171), btn(0, 105, 155, 171), btn(0, 105, 155, 171), btn(0, 105, 155, 190)])
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[0]), btn(0, 105, 155, 194), btn(0, 389, 155, 10), btn(0, 105, 155, 127), btn(0, 105, 155, 195)])
    k.case([open_ge(), btn(0, 105, 155, 190)])
    k.case([open_ge(), a('setOfferItem', p=0, item=1511), btn(0, 105, 155, 190)])
    k.case([open_ge(), btn(0, 105, 155, 999), btn(0, 105, 155, 209), btn(0, 105, 155, 203)])
    k.case(buy(0, 0, 1511, 5, 4) + buy(0, 0, 1511, 5, 4))
    k.case(buy(0, 0, 1511, 5, 4) + [worker()] + buy(0, 0, 1511, 5, 4) + [worker()])
    k.case(buy(0, 5, 1511, 5, 4) + [worker(), view(0, 5)])
    k.case([a('attr', p=0, key='ge-index', value=7)] + buy(0, 0, 1511, 1, 4, confirm=False) + [btn(0, 105, 155, 190), worker()])
    k.case([a('attr', p=0, key='ge-index', value=6), open_ge(), btn(0, 105, 155, 209)])
    k.case([a('attr', p=0, key='ge-index', value=-2), open_ge(), btn(0, 105, 155, 209)])
    # Selling: unnoted, noted, both, too many, coins, untradeable, destroy, the examine option.
    k.case(sell(0, 0, 1, 10, 3))
    k.case(sell(0, 0, 2, 50, 3))
    k.case(sell(0, 0, 1, 300, 3))
    k.case(sell(0, 0, 1, 601, 3))
    k.case(sell(0, 0, 3))
    k.case(sell(0, 0, 0))
    k.case(sell(0, 0, 7))
    k.case(sell(0, 0, 8))
    k.case(sell(0, 0, 9))
    k.case(sell(0, 0, 5, 5000, 1))
    k.case(sell(0, 0, 10, 50, 300) + [worker()])
    k.case(sell(0, 0, 11) + [worker()])
    k.case([open_ge(), btn(0, 105, 155, SELL_SLOT[0]), btn(0, 107, 196, 18, 1), btn(0, 107, 196, 18, 20), btn(0, 107, 155, 17, 1),
            btn(0, 107, 155, 18, -1), btn(0, 107, 155, 18, 28), btn(0, 107, 124, 18, 1), btn(0, 105, 155, 168), btn(0, 105, 155, 162),
            btn(0, 105, 155, 164), btn(0, 105, 155, 166), btn(0, 105, 155, 190)])
    k.case(sell(0, 0, 1, 10, 3) + [worker(), a('close', p=0)] + sell(0, 1, 1, 10, 3) + [worker()])
    k.case(sell(0, 0, 1, 10, 3, confirm=False) + [a('invReplace', p=0, slot=1, id=None), a('invReplace', p=0, slot=2, id=1512, amount=3),
                                                  btn(0, 105, 155, 190)])
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[0]), btn(0, 105, 155, 170), a('input', p=0, int=5), btn(0, 105, 155, 185),
            a('input', p=0, text='2k'), btn(0, 105, 155, 185), a('input', p=0, text='x'), btn(0, 105, 155, 185), a('input', p=0, int=-1)])
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[0]), btn(0, 105, 155, 170), a('closeChatbox', p=0), a('input', p=0, text='5')])
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[0]), btn(0, 105, 155, 170), btn(0, 105, 155, 194), a('input', p=0, text='5')])
    # Dispatch directly: the checks, artificial players, the news.
    for spec in [dict(item=1511, amount=0, value=5), dict(item=1511, amount=3, value=0), dict(item=1511, amount=3, value=5, state='REGISTERED'),
                 dict(item=1511, amount=3, value=5, uid=7), dict(item=1511, amount=3, value=5, sell=True), dict(item=1511, amount=3, value=5)]:
        k.case([a('dispatch', p=0, offer=spec), worker()])
    art = [P('bot', 99, RICH, artificial=True)]
    k.case([a('dispatch', p=0, offer=dict(item=1511, amount=3, value=5, sell=True)), worker(), a('bots'),
            a('dispatch', p=0, offer=dict(item=1511, amount=4, value=5, sell=True)), worker(), a('bots'), a('botStock', item=1511)],
           players=art, sharedUid=55)
    k.case([a('dispatch', p=0, offer=dict(item=1511, amount=9000, value=5, sell=True)), worker(), a('bots')], players=art,
           constants=dict(limit=100))
    k.case(sell(0, 0, 1, 10, 3) + [worker()], players=art)


def matching_cases(k):
    two = [P('alice', 11, RICH), P('bob', 22, RICH)]
    # Prices on both sides, partial fills, order of arrival.
    for sp, bp, sa, ba in [(5, 5, 10, 10), (5, 7, 10, 10), (7, 5, 10, 10), (5, 5, 10, 4), (5, 5, 4, 10), (1, 100, 3, 3), (3, 3, 100, 1)]:
        k.case(sell(0, 0, 1, sa, sp) + [worker()] + buy(1, 0, 1511, ba, bp) + [worker(), a('timer', p=0), a('timer', p=1), a('awaiting', p=0),
                                                                               a('awaiting', p=1)], two)
        k.case(buy(1, 0, 1511, ba, bp) + [worker()] + sell(0, 0, 1, sa, sp) + [worker(), a('timer', p=0), a('timer', p=1)], two)
    k.case(sell(0, 0, 1, 10, 5) + buy(1, 0, 1511, 10, 6) + [worker()], two)
    k.case(sell(0, 0, 1, 10, 5) + buy(1, 0, 1511, 10, 6) + [worker()], two, step=0)
    k.case(buy(1, 0, 1511, 10, 6) + sell(0, 0, 1, 10, 5) + [worker()], two, step=0)
    k.case(buy(1, 0, 1511, 10, 6) + sell(0, 0, 1, 10, 5) + [worker()], two, step=-3)
    # Several sellers: cheapest first; equal prices keep row order.
    three = two + [P('carol', 33, RICH)]
    k.case(sell(0, 0, 1, 5, 8) + [worker()] + sell(1, 0, 1, 5, 6) + [worker()] + sell(2, 0, 1, 5, 7) + [worker()] +
           buy(0, 1, 1511, 12, 9) + [worker(), a('valid'), a('stats', item=1511, sale=False), a('stats', item=1511, sale=True)], three)
    k.case(sell(0, 0, 1, 5, 6) + [worker()] + sell(1, 0, 1, 5, 6) + [worker()] + sell(2, 0, 1, 5, 6) + [worker()] +
           buy(0, 1, 1511, 7, 6) + [worker()], three)
    k.case(buy(0, 0, 1511, 5, 6) + [worker()] + buy(1, 0, 1511, 5, 9) + [worker()] + buy(2, 0, 1511, 5, 7) + [worker()] +
           sell(0, 1, 1, 12, 5) + [worker(), a('stats', item=1511, sale=True)], three)
    # Same account on both sides: no price index update.
    k.case(sell(0, 0, 1, 5, 5) + [worker()] + buy(0, 1, 1511, 5, 5) + [worker(), a('value', item=1511)])
    k.case(sell(0, 0, 1, 5, 5) + [worker()] + buy(1, 0, 1511, 5, 5) + [worker(), a('value', item=1511)], [P('alice', 11, RICH), P('bob', 11, RICH)])
    # The other side offline: no visualisation, no notification.
    k.case(sell(0, 0, 1, 5, 5) + [worker(), a('online', p=0, value=False)] + buy(1, 0, 1511, 5, 5) +
           [worker(), a('online', p=0, value=True), a('timer', p=0), a('timer', p=1)], two)
    # Collection after trades, both slots, notes, other options, history.
    k.case(sell(0, 0, 1, 10, 5) + [worker()] + buy(1, 0, 1511, 4, 7) + [worker(), view(1, 0), collect(1, 0), collect(1, 1), view(0, 0),
                                                                        collect(0, 0, 155), collect(0, 1), btn(0, 105, 155, 203), view(0, 0),
                                                                        collect(0, 0), collect(0, 1), a('history', p=0), a('active', p=0),
                                                                        a('format', p=0), a('save', p=0), a('save', p=1)], two)
    k.case(sell(0, 0, 1, 10, 5) + [worker()] + buy(1, 0, 1511, 10, 5) + [worker(), view(1, 0), collect(1, 0, 155), collect(1, 1, 124),
                                                                         a('history', p=1), a('cacheValue', p=1, slot=0)], two)
    # Six completed trades push the history beyond five entries.
    steps = []
    for i in range(6):
        steps += sell(0, i, 1, 1, 4) + [worker()] + buy(1, i, 1511, 1, 4) + [worker()]
    for i in range(6):
        steps += [view(0, i), collect(0, 0), collect(0, 1), view(1, i), collect(1, 0), collect(1, 1)]
    steps += [a('history', p=0), a('history', p=1), a('save', p=0)]
    k.case(steps, two)
    # Collection box (109): the pulse runs the withdrawal.
    k.case(sell(0, 0, 1, 10, 5) + [worker()] + buy(1, 0, 1511, 10, 5) + [worker(), a('collection', p=1), btn(1, 109, 196, 18, 0),
                                                                         btn(1, 109, 155, 18, 2), a('pulse'), btn(1, 109, 196, 23, 0),
                                                                         btn(1, 109, 196, 36, 0), btn(1, 109, 196, 99, 0), a('pulse'),
                                                                         a('collection', p=0), btn(0, 109, 155, 18, 0), a('pulse')], two)
    k.case([a('collection', p=0)], [P('alice', 11, RICH, pin=False)])
    k.case([a('collection', p=0), btn(0, 109, 196, 52, 3), a('pulse')], [P('alice', 11, RICH, login=False)])
    # Full inventory and the ring replacement.
    full = [(i, 3000 + i, 1) for i in range(27)] + [(27, 995, 1000)]
    k.case(buy(0, 0, 1511, 3, 4) + [worker()] + sell(1, 0, 1, 3, 4) + [worker(), view(0, 0), collect(0, 0), collect(0, 0, 155),
                                                                       a('invReplace', p=0, slot=0, id=None), collect(0, 0)],
           [P('alice', 11, full), P('bob', 22, RICH)])
    k.case(buy(0, 0, 14638, 1, 1000) + [worker()] + sell(1, 0, 3, 1, 1000, confirm=False) + [worker()], [P('alice', 11, RICH), P('bob', 22, RICH)])
    k.case([a('sql', statements=["INSERT INTO player_offers VALUES (40, 11, 14638, 1, 1, 1000, 5, 4, 0, '0,14638,1:1,null,null', 0, 1000)"]),
            a('login', p=0), a('parse', p=0, json='{}'), view(0, 0), collect(0, 0, 155)])
    # Abort: active, completed, null, through the slot option 205.
    k.case(sell(0, 0, 1, 10, 5) + [worker(), view(0, 0), btn(0, 105, 155, 203), view(0, 0), collect(0, 0), btn(0, 105, 155, 203)])
    k.case(buy(0, 0, 1511, 10, 5) + [worker(), view(0, 0, 205), view(0, 0, 205), collect(0, 0), collect(0, 1)])
    k.case(buy(0, 0, 1511, 10, 5) + [view(0, 0, 205), worker(), view(0, 0, 205)])
    k.case(sell(0, 0, 1, 10, 5) + [worker()] + buy(1, 0, 1511, 4, 5) + [worker(), view(0, 0, 205), collect(0, 0), collect(0, 1)], two)
    k.case([open_ge(), btn(0, 105, 155, 203), view(0, 3, 205)])


def bot_cases(k):
    two = [P('alice', 11, RICH), P('bob', 22, RICH)]
    k.case([a('botOffer', item=1511, amount=50), a('botOffer', item=1512, amount=5), a('botOffer', item=7462, amount=5),
            a('botOffer', item=99999, amount=5), worker(), a('bots'), a('botStock', item=1511), a('stats', item=1511, sale=False)]
           + buy(0, 0, 1511, 20, 3) + [worker()] + buy(0, 1, 1511, 20, 10) + [worker(), a('bots'), a('timer', p=0)])
    k.case([a('botOffer', item=1513, amount=5), worker()] + buy(0, 0, 1513, 3, 900) + [worker(), a('value', item=1513)])
    k.case([a('botOffer', item=1513, amount=5), worker()] + buy(0, 0, 1513, 3, 900) + [worker(), a('value', item=1513)],
           constants=dict(influence=False))
    k.case([a('botOffer', item=1511, amount=50), worker()] + buy(0, 0, 1511, 20, 10) + [worker(), a('bots')], constants=dict(limit=5))
    k.case([a('botOffer', item=1511, amount=50), a('botOffer', item=1511, amount=25), worker(), a('bots'), a('botStock', item=1511)])
    k.case([a('botOffer', item=1511, amount=INT_MAX), a('botOffer', item=1511, amount=5), worker(), a('bots')], constants=dict(limit=INT_MAX))
    k.case([a('botOffer', item=1511, amount=10), worker()] + sell(0, 0, 1, 10, 3) + [worker(), a('bots')])
    k.case([a('sql', statements=['INSERT INTO bot_offers VALUES (1511, 0)', 'INSERT INTO bot_offers VALUES (1511, 30)']), a('bots'),
            a('botStock', item=1511)] + buy(0, 0, 1511, 3, 10) + [worker(), a('bots')])
    k.case([a('botOffer', item=554, amount=1000), worker()] + buy(0, 0, 554, 3000, 10) + buy(1, 0, 554, 300, 10) + [worker(), a('bots')], two)
    # Autostock.
    k.case([a('autostock', file='{"offers":[{"item":1511,"qty":5}]}')])
    k.case([a('autostock', file=None)], constants=dict(autostock=True))
    k.case([a('autostock', file='{"offers":[{"item":1511,"qty":5},{"item":"1512","qty":7},{"item":7462,"qty":3},{"item":4151,"qty":2}]}'),
            worker(), a('bots')], constants=dict(autostock=True))
    for text in ['{"offers":[{"item":1511}]}', '{"offers":[{"item":"x","qty":1}]}', '{"offers":[{"item":1.5,"qty":1}]}', '{"other":1}',
                 '{"offers":null}', '{"offers":{}}', '{"offers":[null]}', '{"offers":[5]}', '[1]', 'null', '{oops', '']:
        k.case([a('autostock', file=text), worker(), a('bots')], constants=dict(autostock=True))
    # Commands.
    for name, args in [('addbotoffer', ['addbotoffer', '1511', '12']), ('addbotoffer', ['addbotoffer', '7462', '1']), ('addbotoffer', ['addbotoffer', '1511']),
                       ('addbotoffer', ['addbotoffer', 'x', '1']), ('bange', ['bange', '1511']), ('bange', ['bange']), ('allowge', ['allowge', '7462']),
                       ('allowge', ['allowge', '1511']), ('allowge', ['allowge', '-1'])]:
        k.case([a('command', p=0, name=name, args=args), worker(), a('bots'), a('canTrade', item=1511), a('canTrade', item=7462)])
    # The cheater switch: the offer is written at once and exchanged by a pulse.
    k.case(buy(0, 0, 1511, 5, 4) + [a('pulse'), a('pulse'), worker()], constants=dict(cheater=True))
    k.case(sell(0, 0, 1, 5, 4) + [a('sql', statements=['DELETE FROM player_offers']), a('pulse'), a('pulse')], constants=dict(cheater=True))


def records_cases(k):
    # Saved history, legacy rows without a slot, the login notice.
    for history in ['[]', '[{"itemId":"1511","isSell":true,"totalCoinExchange":"50","completedAmount":"10"}]',
                    '[{"itemId":1511,"isSell":false,"totalCoinExchange":5,"completedAmount":1},{"itemId":"4151","isSell":true,"totalCoinExchange":"9","completedAmount":"2"}]',
                    '[{"itemId":"x","isSell":true,"totalCoinExchange":"1","completedAmount":"1"}]', '[{"isSell":true}]', '[{"itemId":"1"}]',
                    '[{"itemId":"1","isSell":"yes"}]', '[null]', '[5]', '{}', 'null', '"x"',
                    '[' + ','.join('{"itemId":"1511","isSell":true,"totalCoinExchange":"1","completedAmount":"1"}' for _ in range(6)) + ']']:
        k.case([a('parse', p=0, json='{"ge-history":' + history + '}'), a('save', p=0), a('history', p=0)])
        k.case([a('parse', p=0, json='{"ge-history":' + history + '}'), a('save', p=0)], players=[P('alice', 11, RICH, login=False)])
    row = "INSERT INTO player_offers VALUES ({uid}, 11, 1511, 10, {done}, 5, {uid}, {state}, 1, {withdraw}, {slot}, {coins})"
    for slots in [[-1], [-1, -1], [0, -1], [-1] * 6, [-1] * 7, [0, 1, 2, 3, 4, 5], [2, 9], [-2]]:
        seed = [row.format(uid=10 + i, done=i, state=1 + (i % 3), withdraw="'0,995,5:1,null,null'" if i % 2 else 'NULL', slot=s, coins=i * 5)
                for i, s in enumerate(slots)]
        k.case([a('sql', statements=seed), a('parse', p=0, json='{}'), a('active', p=0), a('format', p=0), a('awaiting', p=0)])
    k.case([a('sql', statements=[row.format(uid=10, done=10, state=4, withdraw="'0,null,null:1,1511,10'", slot=0, coins=50)]),
            a('parse', p=0, json='{}'), a('awaiting', p=0), a('timer', p=0)])
    k.case([a('sql', statements=[row.format(uid=10, done=10, state=4, withdraw="'0,1511,10:1,null,null'", slot=1, coins=50)]),
            a('parse', p=0, json='{}'), a('awaiting', p=0)])
    for withdraw in ["''", "'0,995,5'", "'0,995'", "'2,995,5:1,null,null'", "'x,995,5'", "'0,995,y'", "':0,995,5::'", "'0,null'", "12",
                     "'0,995,5:1,995,7'"]:
        k.case([a('sql', statements=[row.format(uid=10, done=1, state=1, withdraw=withdraw, slot=0, coins=5)]), a('parse', p=0, json='{}'),
                view(0, 0)])
    for state in [6, 7, -1, 5]:
        k.case([a('sql', statements=[row.format(uid=10, done=1, state=state, withdraw='NULL', slot=0, coins=5)]), a('parse', p=0, json='{}'),
                a('valid'), view(0, 0)])
    k.case([a('sql', statements=[row.format(uid=10, done=1, state=1, withdraw='NULL', slot=0, coins=5)]), a('parse', p=0, json='{}')],
           players=[P('alice', 11, RICH, login=False)])
    k.case([a('login', p=0), a('timer', p=None), a('active', p=0), a('format', p=0), a('awaiting', p=0)])


def interface_cases(k):
    # Guide prices.
    for kind in ['LOGS', 'ORES', 'RUNES', 'HERBS', 'WEAPONS_AND_ARMOUR']:
        k.case([a('guide', p=0, type=kind)] + [btn(0, 642, 155, b) for b in [15, 23, 24, 43, 57, 89, 103, 135, 144, 167, 182, 0]] +
               [a('pulse'), btn(0, 642, 196, 15), a('pulse')])
    k.case([a('guide', p=0, type='LOGS'), a('guide', p=0, type='HERBS'), btn(0, 642, 155, 150), a('pulse')])
    k.case([a('open', p=0, iface=642), btn(0, 642, 155, 15), a('pulse')])
    # Item sets.
    sets_inv = [(0, 995, 1000), (1, 11814, 1), (2, 1155, 1), (3, 1117, 1), (4, 1075, 1), (5, 1189, 1), (6, 11967, 2), (7, 11842, 1)]
    cs2 = {'1089': {'11814': 'Bronze helm, body, legs, kiteshield', '11842': 'Dragon parts'}}
    k.case([a('sets', p=0), btn(0, 644, 196, 0, 1), a('pulse'), btn(0, 645, 196, 16, 0), a('pulse'), btn(0, 645, 196, 16, 0), a('pulse'),
            btn(0, 644, 124, 0, 6), btn(0, 645, 124, 16, 69), btn(0, 644, 155, 0, 1), btn(0, 645, 155, 16, 14), btn(0, 645, 155, 16, 69), a('pulse'),
            btn(0, 644, 196, 0, 6), a('pulse'), btn(0, 645, 196, 16, 14), a('pulse'), a('close', p=0), a('invAdd', p=0, id=590, amount=1)],
           [P('alice', 11, sets_inv)], table='sets', cs2=cs2)
    full = sets_inv + [(i, 3000 + i - 8, 1) for i in range(8, 26)]
    k.case([a('sets', p=0), btn(0, 644, 196, 0, 1), btn(0, 644, 196, 0, 9), btn(0, 644, 196, 3, 1), btn(0, 644, 196, 0, 30),
            btn(0, 645, 196, 16, 70), btn(0, 645, 196, 16, -1), a('pulse'), a('invReplace', p=0, slot=24, id=None), btn(0, 644, 196, 0, 1), a('pulse')],
           [P('alice', 11, full)], table='sets')
    k.case([a('sets', p=0), btn(0, 644, 155, 0, 1), a('pulse')], [P('alice', 11, sets_inv)], table='sets')
    k.case([a('sets', p=0), btn(0, 644, 155, 0, 1), a('pulse')], [P('alice', 11, sets_inv)], table='sets', cs2={'1089': {}})
    k.case([a('sets', p=0), a('invAdd', p=0, id=590, amount=1), a('sets', p=0), a('close', p=0), a('invAdd', p=0, id=590, amount=1)],
           [P('alice', 11, sets_inv)], table='sets', containerIndex=6997)
    k.case([btn(0, 644, 196, 0, 1), a('pulse')], [P('alice', 11, sets_inv)], table='sets')
    k.case([a('open', p=0, iface=645), a('close', p=0)], [P('alice', 11, sets_inv)], table='sets')
    # Search interface and the side buttons without an open interface.
    k.case([open_ge(), btn(0, 105, 155, BUY_SLOT[0]), btn(0, 389, 155, 10), a('pulse'), btn(0, 389, 155, 11), a('pulse')])
    k.case([btn(0, 105, 155, 190), btn(0, 107, 155, 18, 1)])


def failure_cases(k):
    two = [P('alice', 11, RICH), P('bob', 22, RICH)]
    scenario = (sell(0, 0, 1, 10, 5) + [worker()] + buy(1, 0, 1511, 4, 7) + [worker(), a('timer', p=0), view(1, 0), collect(1, 0),
                                                                            a('collection', p=1), btn(1, 109, 196, 18, 2), a('pulse'),
                                                                            btn(0, 105, 155, 203), a('history', p=1), a('save', p=1),
                                                                            a('botOffer', item=1511, amount=5), worker(), a('guide', p=0, type='ORES'),
                                                                            a('stats', item=1511, sale=False), worker()])
    for event, ats in [('sql', [1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144]), ('send', [1, 3, 10, 30, 60]), ('message', [1]), ('contentMessage', [1, 3]),
                       ('setVarp', [1, 9, 20]), ('audio', [1, 2]), ('jingle', [1]), ('log', [1]), ('logGE', [1]), ('news', [1]), ('discord', [1, 2]),
                       ('clock', [1, 2, 3, 4]), ('uidMap', [1, 2]), ('players', [1, 3, 9]), ('pinUnlocked', [1]), ('examine', [1]),
                       ('submit', [1]), ('invUpdate', [1, 4]), ('sendInput', [1]), ('buttonClick', [3]), ('dispatchOpen', [1]),
                       ('dispatchClose', [1]), ('workerStart', [1])]:
        for at in ats:
            k.case(scenario, two, failures={event: at})
    k.case([a('sql', statements=["INSERT INTO player_offers VALUES (40, 11, 1511, 1, 0, 5, 5, 9, 0, NULL, 0, 0)"])] + sell(1, 0, 1, 1, 5)
           + [worker()] + buy(1, 1, 1511, 1, 5) + [worker(), worker()], two)
    k.case([a('attr', p=0, key='ge-index', value=6)] + sell(0, 0, 1, 1, 5, confirm=False) + [btn(0, 105, 155, 190), worker()] +
           buy(1, 0, 1511, 1, 5) + [worker(), worker()], two)
    k.case(buy(0, 0, 1511, 1, 5) + [worker()], [P('alice', 11, RICH, login=False)])
    k.case(buy(0, 0, 1511, 1, 5) + [worker()], failures={'sql': 4}, startup='full')
    k.case([a('dispatch', p=0, offer=dict(item=1511, amount=1, value=5)), worker(), a('dispatch', p=0, offer=dict(item=1511, amount=1, value=5)),
            worker()], failures={'clock': 1})
    k.case([a('command', p=0, name='addbotoffer', args=['addbotoffer', '1511', '5'])], failures={'notify': 1})
    # A failure inside GEDB.run while a result set is still open (the bot match, a uid read, a stats loop): the
    # provider's connection is never closed again.
    bot = [a('botOffer', item=1511, amount=50), worker()] + buy(0, 0, 1511, 5, 10) + [worker()]
    after = [a('botOffer', item=1513, amount=5), worker(), a('ban', item=1925), a('allow', item=12345), a('addTrade', item=1511, amount=2, price=3),
             a('value', item=1511), a('canTrade', item=1925), a('bots'), a('valid')] + sell(0, 1, 1, 3, 4) + [worker()]
    for event, at in [('uidMap', 1), ('uidMap', 2), ('clock', 6), ('send', 50), ('players', 2), ('sql', 30), ('sql', 33)]:
        k.case(bot + after, failures={event: at})
    k.case(sell(0, 0, 1, 2, 4) + [worker(), a('stats', item=1511, sale=False)] + after, failures={'players': 3})
    k.case(sell(0, 0, 1, 2, 4) + [worker(), view(0, 0)] + after, failures={'players': 2})


def corpus():
    k = Corpus()
    setup_cases(k)
    price_cases(k)
    offer_cases(k)
    matching_cases(k)
    bot_cases(k)
    records_cases(k)
    interface_cases(k)
    failure_cases(k)
    return k
