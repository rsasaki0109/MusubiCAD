#!/usr/bin/env python3
"""Independent P12-007 artifact validation; install ezdxf==1.4.4 first."""
from pathlib import Path
import math
import ezdxf

ROOT = Path(__file__).resolve().parents[1]

def close(a, b):
    assert math.isfinite(a) and abs(a-b) < 1e-6, (a, b)

def load(name):
    doc = ezdxf.readfile(ROOT / 'examples' / f'{name}_plate_drawing.dxf')
    assert doc.dxfversion == 'AC1027' and doc.units == 4 and doc.header['$MEASUREMENT'] == 1
    audit = doc.audit()
    assert not audit.errors and not audit.fixes, [(e.code, e.message) for e in audit.errors + audit.fixes]
    paper = list(doc.modelspace().query('LWPOLYLINE[layer=="PAPER"]'))
    assert len(paper) == 1 and paper[0].closed
    assert list(paper[0].get_points('xy')) == [(0.0,0.0),(210.0,0.0),(210.0,297.0),(0.0,297.0)]
    return doc.modelspace()

def main():
    title = load('title')
    labels = [e.plain_text() for e in title.query('TEXT')]
    assert 'Inspection plate' in labels and 'MUSUBI-P12-005' in labels and 'FRONT 1:2' in labels
    xs, ys = [], []
    for e in title.query('LINE'):
        if e.dxf.layer in ('VISIBLE','HIDDEN'):
            xs += [e.dxf.start.x, e.dxf.end.x]
            ys += [e.dxf.start.y, e.dxf.end.y]
    for actual, expected in [(min(xs),50),(max(xs),90),(min(ys),100),(max(ys),130)]: close(actual,expected)
    section = load('section')
    contours = list(section.query('LWPOLYLINE[layer=="SECTIONS"]'))
    assert len(contours) == 2 and all(e.closed for e in contours)
    def area(e):
        points = list(e.get_points('xy'))
        return abs(sum(a[0]*b[1]-a[1]*b[0] for a,b in zip(points,points[1:]+points[:1]))/2)
    areas = sorted(map(area,contours),reverse=True)
    close(areas[0]-areas[1],1100) # (80*60-20*20) mm² at 1:2 scale
    hatches = list(section.query('LINE[layer=="HATCH"]'))
    assert hatches
    for e in hatches:
        a,b=e.dxf.start,e.dxf.end
        close(b.x-a.x,b.y-a.y)
        x,y=(a.x+b.x)/2,(a.y+b.y)/2
        assert not (65+1e-6<x<75-1e-6 and 60+1e-6<y<70-1e-6)
        phase=(a.x-a.y+297)/(3*math.sqrt(2))
        close(phase,round(phase))
    print('Independent ezdxf audit: units, paper, analytic dimensions/section area and clear hole passed')

if __name__ == '__main__':
    main()
