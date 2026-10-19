"""Independent SI/source oracle for ADR-112; never trust a cached mass report."""
import copy
import json
import math
from sketch_creation_eval import physical_problems, triangles
from arc_sector_eval import graph_source


def physical(data, plane='XY', degrees=90, axis_mm=5, negative_side=False, negative_axis=False):
    def mapped(u, v):
        return {'XY': (u, v, 0), 'YZ': (0, u, v), 'XZ': (u, 0, v)}[plane]
    sign = -1 if negative_side else 1
    axis_sign = -1 if negative_axis else 1
    # The rectangle endpoints are fixed source coordinates; moving the datum
    # changes both radii, rather than translating a cached bushing.
    endpoints = [15, 25] if not negative_side else [-5, -15]
    a, b = sorted(abs(v-axis_mm)/1000 for v in endpoints)
    t, h = math.radians(degrees), .02
    x, z = mapped(sign, 0), mapped(0, axis_sign)
    y = (z[1]*x[2]-z[2]*x[1], z[2]*x[0]-z[0]*x[2], z[0]*x[1]-z[1]*x[0])
    origin = mapped(axis_mm/1000, .013 if negative_axis else -.007)
    def point(r, angle, height):
        return tuple(origin[i]+z[i]*height+r*(x[i]*math.cos(angle)+y[i]*math.sin(angle)) for i in range(3))
    angles = [0, t]+[a for a in [math.pi/2, math.pi, 3*math.pi/2, 2*math.pi] if a <= t+1e-14]
    points = [point(r, a, v) for r in [a, b] for a in angles for v in [0, h]]
    bounds = tuple(tuple(fn(p[i] for p in points) for i in range(3)) for fn in [min, max])
    d = 2*(b**3-a**3)/(3*(b*b-a*a)*t)
    center = tuple(origin[i]+z[i]*h/2+(0 if degrees == 360 else d*(x[i]*math.sin(t)+y[i]*(1-math.cos(t)))) for i in range(3))
    problems = physical_problems(data, bounds=bounds, expected_center=center, expected_volume=(b*b-a*a)*t*h/2)
    if problems:
        return problems
    actual = [p for face in triangles(data) for p in face]
    for radius in [a, b]:
        for height in [0, h]:
            for angle in [0, t]:
                wanted = point(radius, angle, height)
                if not any(all(abs(p[i]-wanted[i]) < 3e-7 for i in range(3)) for p in actual):
                    return ['revolve is missing an exact start/end radial/axial contact']
    for p in actual:
        q = [p[i]-origin[i] for i in range(3)]
        radial = sum(q[i]*x[i] for i in range(3))
        tangent = sum(q[i]*y[i] for i in range(3))
        r = math.hypot(radial, tangent)
        angle = math.atan2(tangent, radial) % (2*math.pi)
        if angle > 2*math.pi-1e-6:
            angle = 0
        if r < a-1e-4 or r > b+1e-4 or angle > t+1e-6:
            return ['revolve radial interval or positive angular branch differs']
    return []


def patch_source_problems(document, patch):
    """Verify actual source against the authored recipe, not against itself."""
    try:
        g = graph_source(document)
        ops = patch['operations']
        nodes = [o['node'] for o in ops if o['type'] == 'add_feature']
        actual = g['features']['feature_nodes']
        if len(actual) != len(nodes):
            return ['revolve recipe lost or added feature sources']
        for wanted in nodes:
            found = next(n for n in actual if n['id'] == wanted['id'])
            if any(found.get(k) != v for k, v in wanted.items()) or found.get('suppressed', False):
                return ['revolve feature identity, datum association or expression differs']
        seeds = [o for o in ops if o['type'] == 'add_sketch']
        if {s['id'] for s in g['sketches']['sketches']} != {s['id'] for s in seeds} or len(g['sketches']['sketches']) != len(seeds):
            return ['revolve recipe sketch inventory differs']
        if len(g['parameters']['parameters']) != len([o for o in ops if o['type'] == 'add_parameter']):
            return ['revolve recipe parameter inventory differs']
        if g['semantic_refs'] != {'semantic_refs': []}:
            return ['revolve recipe changed semantic references']
        for sketch in g['sketches']['sketches']:
            seed = next(o for o in ops if o['type'] == 'add_sketch' and o['id'] == sketch['id'])
            expected = [o['entity'] for o in ops if o['type'] == 'add_sketch_entity' and o['sketch_id'] == sketch['id']]
            if sketch['workplane'] != seed['workplane'] or sorted(sketch['entities'], key=lambda e:e['id']) != sorted(expected, key=lambda e:e['id']) or sketch['constraints']:
                return ['revolve rectangle source or global plane differs']
        for o in [o for o in ops if o['type'] == 'add_parameter']:
            actual = g['parameters']['parameters'][o['id']]
            if actual['name'] != o['name'] or actual['expr'] != o['expr']:
                return ['revolve parameter source differs']
        expected_order = [n['id'] for n in nodes]
        if g['features']['feature_graph']['order'] != expected_order:
            return ['revolve feature order differs']
        edges = {(e['source'], e['target']) for e in g['features']['feature_graph']['edges']}
        if edges != {('feature:sketch_revolve_profile', 'feature:physical_revolve'), ('feature:revolve_axis', 'feature:physical_revolve')}:
            return ['revolve profile/axis dependencies differ']
        return []
    except (OSError, ValueError, KeyError, TypeError, StopIteration):
        return ['revolve authored source evidence unavailable']


def source_problems(document, desired, baseline):
    try:
        current = graph_source(document)
        before = copy.deepcopy(baseline)
        for data in [current, before]:
            for p in data['parameters']['parameters'].values():
                p.pop('dirty', None)
        for id, expr in desired['parameters'].items():
            before['parameters']['parameters'][id]['expr'] = expr
        if current != before:
            return ['revolve edit changed profile, datum association, angle source or unrelated intent']
        return []
    except (OSError, ValueError, KeyError, TypeError):
        return ['revolve edit source evidence unavailable']
