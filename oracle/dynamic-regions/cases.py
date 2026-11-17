"""The dynamic-region corpus: real cache regions copied, rotated, entered, left and removed.

Instances are reserved from the south-west corner of the map, so the first one
of a case occupies region 0 (tiles 0-63) and later ones follow the original's
search order.
"""

LUMBRIDGE = 12850
TEMPLATES = [12850, 9551, 10536, 8011, 11573, 12854]
SAMPLES = [(7, 17), (20, 20), (32, 32), (10, 50), (63, 0), (0, 63), (45, 12), (5, 5)]


def base(region):
    return ((region >> 8) << 6, (region & 0xFF) << 6)


def at(region, x, y, z=0):
    bx, by = base(region)
    return [bx + x, by + y, z]


def probe(region, instance_base=(0, 0)):
    out = []
    for x, y in SAMPLES:
        for z in [0, 1]:
            out.append(dict(op='object', location=[instance_base[0] + x, instance_base[1] + y, z]))
            out.append(dict(op='clip', location=[instance_base[0] + x, instance_base[1] + y, z]))
            out.append(dict(op='clip', location=at(region, x, y, z)))
    return out


def create_cases():
    out = []
    for template in TEMPLATES:
        actions = [dict(op='create', template=template, as_='A')]
        actions += probe(template)
        actions += [
            dict(op='player', name='p', location=[32, 32, 0]),
            dict(op='scene', name='p', login=False),
            dict(op='player', name='q', location=at(template, 32, 32)),
            dict(op='scene', name='q', login=True),
            dict(op='minimap', name='p', state=2),
            dict(op='clearFlag', name='p'),
            dict(op='minimap', name='p', state=0),
            dict(op='leave', name='p'),
            dict(op='tick', count=49),
            dict(op='tick', count=1),
            dict(op='tick', count=1),
        ]
        out.append(dict(group='create', name=f'create {template}', actions=actions))
    return out


def npc_cases():
    lum = LUMBRIDGE
    template_npcs = [
        dict(op='player', name='owner', location=at(lum, 40, 40)),
        dict(op='npc', name='guard', id=1, location=at(lum, 10, 10), direction='SOUTH', aggressive=True, respawn=True, walks=True),
        dict(op='npc', name='cow', id=81, location=at(lum, 11, 10, 1), direction='NORTH_EAST'),
        dict(op='npc', name='pet', id=6969, location=at(lum, 12, 10), owner='owner'),
    ]
    out = [dict(group='npcs', name='template NPCs are copied onto the unregistered instance', actions=template_npcs + [
        dict(op='create', template=lum, as_='A'),
        dict(op='player', name='p', location=[10, 10, 0]),
        dict(op='leave', name='p'),
        dict(op='tick', count=50),
    ])]
    instance_npcs = [
        dict(op='player', name='owner', location=at(lum, 40, 40)),
        dict(op='create', template=lum, as_='A'),
        dict(op='npc', name='man', id=2, location=[20, 20, 0]),
        dict(op='npc', name='sleeper', id=3, location=[21, 20, 0], active=False),
        dict(op='npc', name='fam', id=6969, location=[22, 20, 2], owner='owner'),
        dict(op='player', name='p', location=[25, 25, 0]),
        dict(op='leave', name='p'),
    ]
    out.append(dict(group='npcs', name='instance NPCs are cleared and familiars recalled', actions=instance_npcs + [
        dict(op='tick', count=50),
        dict(op='flagInactive', region='A', force=True),
    ]))
    out.append(dict(group='npcs', name='a familiar recall failure stops the clearing', actions=instance_npcs + [
        dict(op='fail', service='familiar', at=1),
        dict(op='flagInactive', region='A', force=False),
    ]))
    out.append(dict(group='npcs', name='an NPC clear failure stops the clearing', actions=instance_npcs + [
        dict(op='fail', service='npcClear', at=2),
        dict(op='flagInactive', region='A', force=False),
        dict(op='flagInactive', region='A', force=False),
    ]))
    out.append(dict(group='npcs', name='the public NPC list is cleared', actions=instance_npcs + [
        dict(op='track', region='A', name='man'),
        dict(op='track', region='A', name='sleeper'),
        dict(op='clear', region='A'),
        dict(op='clear', region='A'),
    ]))
    out.append(dict(group='npcs', name='a null NPC fails activation and clearing', actions=[
        dict(op='create', template=lum, as_='A'),
        dict(op='nullNpc', region='A', chunk=[1, 1, 0]),
        dict(op='player', name='p', location=[9, 9, 0]),
        dict(op='flagInactive', region='A', force=True),
    ]))
    out.append(dict(group='npcs', name='copying fails at an NPC', actions=template_npcs + [
        dict(op='fail', service='npcInit', at=2),
        dict(op='create', template=lum, as_='A'),
        dict(op='find', size=[8, 8]),
    ]))
    out.append(dict(group='npcs', name='creating an NPC copy fails', actions=template_npcs + [
        dict(op='fail', service='npcCreate', at=1),
        dict(op='create', template=lum, as_='A'),
        dict(op='create', template=9551, as_='B'),
    ]))
    return out


def allocation_cases():
    lum = LUMBRIDGE
    out = [dict(group='allocation', name='areas follow the original search', actions=[
        dict(op='find', size=[8, 8]),
        dict(op='create', template=lum, as_='A'),
        dict(op='create', template=lum, as_='B'),
        dict(op='create', template=9551, as_='C'),
        dict(op='find', size=[8, 8]),
        dict(op='reserve', size=[16, 16], as_='R'),
        dict(op='reserve', size=[4, 4]),
        dict(op='find', size=[24, 8]),
        dict(op='flagInactive', region='B', force=True),
        dict(op='find', size=[8, 8]),
        dict(op='create', template=10536, as_='D'),
        dict(op='removeRegion', region='C'),
        dict(op='create', template=lum, as_='E'),
    ])]
    # Areas 128 wide leave the sixteenth probe of a row free: the row length shows.
    out.append(dict(group='allocation', name='wide areas fill a row', actions=[dict(op='reserve', size=[16, 8]) for _ in range(5)] + [
        dict(op='find', size=[16, 8]),
        dict(op='reserve', size=[16, 8]),
        dict(op='find', size=[8, 16]),
        dict(op='create', template=lum, as_='A'),
    ]))
    rows = [dict(op='reserve', size=[8, 8]) for _ in range(16)]
    out.append(dict(group='allocation', name='the sixteenth probe starts a new row', actions=rows + [dict(op='create', template=lum, as_='A')]))
    out.append(dict(group='allocation', name='several regions share one area', actions=[
        dict(op='createBetween', one=13114, two=13115, as_='W'),
        dict(op='createArea', borders=[3264, 3712, 3328, 3840], as_=['X', 'Y']),
        dict(op='createArea', borders=[3200, 3200, 3272, 3264], as_=['Z']),
        dict(op='createArea', borders=[3200, 3200, 3200, 3264], as_=[]),
        dict(op='player', name='p', location=[30, 100, 0]),
        dict(op='scene', name='p'),
        dict(op='player', name='p2', location=[30, 60, 1]),
        dict(op='scene', name='p2'),
        dict(op='flagInactive', region='X', force=True),
        dict(op='flagInactive', region='Y', force=True),
    ]))
    out.append(dict(group='allocation', name='an empty instance from borders', actions=[
        dict(op='withBorders', borders=[640, 640, 704, 704], as_='A'),
        dict(op='object', location=[650, 650, 0]),
        dict(op='clip', location=[650, 650, 0]),
        dict(op='player', name='p', location=[650, 650, 0]),
        dict(op='scene', name='p'),
        dict(op='flagInactive', region='A', force=True),
    ]))
    out.append(dict(group='allocation', name='an unregistered copy', actions=[
        dict(op='copy', template=lum, to=[1280, 1280, 0], as_='A'),
        dict(op='object', location=[1287, 1297, 0]),
        dict(op='multicombat', region='A'),
        dict(op='flagInactive', region='A', force=True),
    ]))
    return out


def rotation_cases():
    lum = LUMBRIDGE
    out = []
    for build in [True, False]:
        actions = [
            dict(op='load', region=lum, build=build),
            dict(op='npc', name='visitor', id=5, location=at(lum, 18, 18), direction='WEST'),
            dict(op='emptyInstance', as_='E', build=build),
        ]
        sources = [(2, 2, 0), (1, 2, 0), (2, 1, 0), (3, 3, 0), (2, 2, 1), (4, 2, 0)]
        directions = ['EAST', 'SOUTH', 'WEST', 'NORTH', 'NORTH_EAST', 'SOUTH_WEST']
        for i, (source, direction) in enumerate(zip(sources, directions)):
            z = source[2]
            actions.append(dict(op='replace', region='E', z=z, x=i, y=0, source=list(source), **{'from': lum}))
            actions.append(dict(op='rotate', region='E', chunk=[i, 0, z], direction=direction))
        actions.append(dict(op='rotate', region='E', chunk=[0, 0, 0], direction='EAST'))
        actions.append(dict(op='replace', region='E', z=0, x=0, y=1, source=[3, 3, 0], **{'from': lum}))
        for x in range(0, 48, 5):
            for y in [0, 3, 6, 9]:
                actions.append(dict(op='object', location=[x, y, 0]))
        actions += [dict(op='clip', location=[x, 4, 0]) for x in range(0, 48, 3)]
        actions += [
            dict(op='player', name='p', location=[4, 4, 0]),
            dict(op='scene', name='p'),
            dict(op='flagInactive', region='E', force=True),
        ]
        out.append(dict(group='rotation', name=f'house chunks rotated (build {build})', actions=actions))
    out.append(dict(group='rotation', name='a template replaced into a created instance', actions=[
        dict(op='create', template=9551, as_='A'),
        dict(op='replace', region='A', z=0, x=3, y=3, source=[2, 2, 0], **{'from': lum}),
        dict(op='rotate', region='A', chunk=[3, 3, 0], direction='SOUTH'),
        dict(op='player', name='p', location=[28, 28, 0]),
        dict(op='scene', name='p'),
    ]))
    return out


def activity_cases():
    lum = LUMBRIDGE
    house = [
        dict(op='load', region=lum, build=True),
        dict(op='emptyInstance', as_='H', build=True),
        dict(op='emptyInstance', as_='D', build=True),
        dict(op='replace', region='H', z=0, x=1, y=1, source=[2, 2, 0], **{'from': lum}),
        dict(op='replace', region='D', z=0, x=1, y=1, source=[3, 3, 0], **{'from': lum}),
        dict(op='link', region='H', linked=['D']),
    ]
    out = [dict(group='activity', name='a house and its dungeon', actions=house + [
        dict(op='player', name='p', location=[10, 10, 0]),
        dict(op='leave', name='p'),
        dict(op='flagInactive', region='D', force=False),
        dict(op='tick', count=50),
        dict(op='flagInactive', region='D', force=False),
    ])]
    out.append(dict(group='activity', name='an active dungeon holds the house', actions=house + [
        dict(op='player', name='p', location=[138, 10, 0]),
        dict(op='flagInactive', region='H', force=False),
        dict(op='isInactive', region='D', run=False),
        dict(op='leave', name='p'),
        dict(op='flagInactive', region='H', force=False),
    ]))
    out.append(dict(group='activity', name='a refusing linked region does not stop the next', actions=[
        dict(op='load', region=lum, build=True),
        dict(op='emptyInstance', as_='H', build=True),
        dict(op='emptyInstance', as_='D1', build=True),
        dict(op='emptyInstance', as_='D2', build=True),
        dict(op='emptyInstance', as_='D3', build=True),
        dict(op='replace', region='D2', z=0, x=1, y=1, source=[2, 2, 0], **{'from': lum}),
        dict(op='link', region='D1', linked=['D3']),
        dict(op='link', region='H', linked=['D1', 'D2']),
        dict(op='player', name='p', location=[394, 10, 0]),
        dict(op='flagInactive', region='H', force=True),
        dict(op='leave', name='p'),
        dict(op='flagInactive', region='D1', force=True),
    ]))
    out.append(dict(group='activity', name='viewers and players refuse unloading', actions=[
        dict(op='create', template=lum, as_='A'),
        dict(op='view', region='A', name='watcher', add=True),
        dict(op='flagInactive', region='A', force=False),
        dict(op='view', region='A', name='watcher', add=False),
        dict(op='player', name='p', location=[5, 5, 0]),
        dict(op='flagInactive', region='A'),
        dict(op='unload', region='A', force=False),
        dict(op='timeout', region='A', ticks=3),
        dict(op='move', name='p', to=[3205, 3205, 0]),
        dict(op='tick', count=2),
        dict(op='tick', count=1),
        dict(op='checkInactive', region='A'),
        dict(op='checkInactive', region='A'),
    ]))
    out.append(dict(group='activity', name='permanent instances stay', actions=[
        dict(op='create', template=lum, as_='A'),
        dict(op='permanent', region='A', value=True),
        dict(op='flagInactive', region='A', force=True),
        dict(op='removeRegion', region='A'),
        dict(op='permanent', region='A', value=False),
        dict(op='flagInactive', region='A', force=True),
    ]))
    out.append(dict(group='activity', name='multiway combat and music', actions=[
        dict(op='create', template=lum, as_='A'),
        dict(op='multicombat', region='A'),
        dict(op='music', region='A', music=588),
        dict(op='multicombat', region='A'),
        dict(op='multicombat', region='A'),
        dict(op='music', region='A', music=473),
        dict(op='flagInactive', region='A', force=True),
        dict(op='multicombat', region=lum),
        dict(op='link', region=lum, linked=['A']),
    ]))
    out.append(dict(group='activity', name='ground items are dropped with the chunk', actions=[
        dict(op='create', template=lum, as_='A'),
        dict(op='item', region='A', chunk=[2, 2, 0], label='coins'),
        dict(op='item', region=lum, chunk=[2, 2, 0], label='bones'),
        dict(op='flagInactive', region='A', force=True),
        dict(op='flagInactive', region=lum, force=True),
    ]))
    out.append(dict(group='activity', name='removing an occupied instance', actions=[
        dict(op='create', template=lum, as_='A'),
        dict(op='player', name='p', location=[30, 30, 0]),
        dict(op='scene', name='p'),
        dict(op='removeRegion', region='A'),
        dict(op='scene', name='p'),
        dict(op='create', template=9551, as_='B'),
        dict(op='scene', name='p'),
    ]))
    return out


def scene_cases():
    lum = LUMBRIDGE
    out = []
    for name, location in [('edge', [3, 3, 0]), ('upper', [40, 20, 1]), ('east', [60, 33, 0]), ('north', [33, 61, 3])]:
        out.append(dict(group='scene', name=f'build area at the {name}', actions=[
            dict(op='create', template=lum, as_='A'),
            dict(op='create', template=9551, as_='B'),
            dict(op='player', name='p', location=location),
            dict(op='scene', name='p'),
            dict(op='move', name='p', to=[location[0] + 128, location[1], location[2]]),
            dict(op='scene', name='p', login=True),
            dict(op='move', name='p', to=at(lum, 5, 5)),
            dict(op='scene', name='p'),
        ]))
    return out


def failure_cases():
    lum = LUMBRIDGE
    return [dict(group='scene', name='a failed write keeps the last scene', actions=[
        dict(op='create', template=lum, as_='A'),
        dict(op='player', name='p', location=[30, 30, 0]),
        dict(op='fail', service='session', at=1),
        dict(op='scene', name='p'),
        dict(op='scene', name='p'),
        dict(op='move', name='p', to=at(lum, 9, 9)),
        dict(op='fail', service='session', at=1),
        dict(op='scene', name='p'),
        dict(op='scene', name='p'),
    ])]


def requests():
    cases = create_cases() + npc_cases() + allocation_cases() + rotation_cases() + activity_cases() + scene_cases() + failure_cases()
    for case in cases:
        for action in case['actions']:
            if 'as_' in action:
                action['as'] = action.pop('as_')
    return cases


def groups(cases):
    out = {}
    for case in cases:
        out[case['group']] = out.get(case['group'], 0) + 1
    return dict(sorted(out.items()))
