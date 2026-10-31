"""Job tracking, round 1: three ways to draw a tracked job's walk to its target, animated
at the game's 30 ticks a second over the game's own street frames.

    SHOTS=dir/ python3 -I gen.py out.html

SHOTS holds tmp_track_{near,far}_120x40.svg and tmp_track.json, from a throwaway shot
test in screens/shots.rs: street_by_the_body(), then VEX moved back along the A* walk
from home to Marlo's body, 30 steps (near: the body on screen) and 95 steps (far: off
the top of the map). The json holds each frame's player tile, the body and the A* path
from the player to it.

Jeffrey picked C (floor glow) on 2026-10-31 and asked to keep the page for inspiration:
round1.html is it, with A (trace) and B (runway) beside the pick."""

import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid, CSS  # noqa: E402

S = os.environ.get("SHOTS", "shots/")
W, H = 120, 40
# The map's cells in a 120x40 frame, and the player's cell (the camera's middle).
MAP_X0, MAP_X1, MAP_Y0, MAP_Y1 = 0, 79, 5, 36
PX, PY = 39, 20

CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
PANEL = "#04060a"
FAINT = "#5c6673"
DIM = "#a0aab9"

# Cells the line may draw over: bare ground. Anything else (people, bodies, litter,
# props) keeps its glyph and only takes the line's tint behind it.
GROUND = set(" ░⠁~·")

LINE = {
    frozenset("LR"): "━", frozenset("UD"): "┃",
    frozenset("RD"): "┏", frozenset("LD"): "┓",
    frozenset("UR"): "┗", frozenset("UL"): "┛",
}
CHEVRON = {"R": "▸", "L": "◂", "U": "▴", "D": "▾"}


def side(a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    return {(1, 0): "R", (-1, 0): "L", (0, -1): "U", (0, 1): "D"}[(dx, dy)]


def scene(name, data):
    g = Grid.from_svg(os.path.join(S, f"tmp_track_{name}_{W}x{H}.svg"), W, H)
    px, py = data["player"]
    path = [tuple(p) for p in data["path"]]

    def screen(t):
        return (PX + t[0] - px, PY + t[1] - py)

    def on_map(c):
        return MAP_X0 <= c[0] < MAP_X1 and MAP_Y0 <= c[1] < MAP_Y1

    cells, exit_cell = [], None
    for i, t in enumerate(path):
        if i == 0:
            continue  # the player's own tile
        c = screen(t)
        if not on_map(c):
            exit_cell = exit_cell or screen(path[i - 1])
            break
        sides = {side(t, path[i - 1])}
        if i + 1 < len(path):
            sides.add(side(t, path[i + 1]))
        if len(sides) == 1:
            s = next(iter(sides))
            sides = {"L", "R"} if s in "LR" else {"U", "D"}
        ch = g.c[c[1]][c[0]][0]
        cells.append({
            "x": c[0], "y": c[1], "i": i,
            "ground": ch in GROUND and i < len(path) - 1,
            "line": LINE[frozenset(sides)],
            "chev": CHEVRON[side(t, path[i + 1])] if i + 1 < len(path) else "",
        })
    target = screen(path[-1])
    left = len(path) - 1
    out = {
        "base": g.c, "cells": cells, "total": left,
        "target": list(target) if on_map(target) else None,
        "exit": None, "ground": [],
    }
    if exit_cell:
        steps_shown = cells[-1]["i"] if cells else 0
        out["exit"] = {"x": exit_cell[0], "y": exit_cell[1], "edge": "U" if exit_cell[1] == MAP_Y0 else
                       "D" if exit_cell[1] == MAP_Y1 - 1 else "L" if exit_cell[0] == MAP_X0 else "R",
                       "left": left - steps_shown}
    if out["target"]:
        # the target's ring: ground cells around it, for the reticle and the ripple
        tx, ty = target
        for dy in range(-3, 4):
            for dx in range(-4, 5):
                c = (tx + dx, ty + dy)
                if on_map(c):
                    out["ground"].append([c[0], c[1], g.c[c[1]][c[0]][0] in GROUND])
    # The JOBS pane: a line under the story's objective says it is tracked.
    rows = [list(map(list, g.c[y][80:])) for y in range(H)]
    for y in range(14, 10, -1):
        g.c[y][80:] = [list(c) for c in rows[y - 1]]
    for x in range(80, W):
        g.c[10][x] = [" ", DIM, PANEL, False]
    tag = f" ⌖ tracking · {left} m"
    for k, ch in enumerate(tag):
        g.c[10][80 + k] = [ch, CYAN if k < 2 else DIM, PANEL, k < 2]
    return out


CONCEPTS = [
    ("A", "Trace", "A heavy circuit line laid along the walk, breathing softly. A bright head "
     "runs from you to the target with a fading tail, then a beat of rest. A reticle "
     "pulses around the target."),
    ("B", "Runway", "Chevrons on every other tile point the way. A wave of light sweeps "
     "them from you outward, like runway lights; two waves in flight. Brackets pulse "
     "either side of the target."),
    ("C", "Floor glow", "No glyphs: the walk's tiles glow from below, and a bright band "
     "slides along them with a trail. Rings ripple out of the target area. Everything on "
     "the street keeps its own glyph."),
]

JS = r"""
const CYAN=[0,229,255], MAG=[255,0,200], WHITE=[234,255,255];
function hx(c){return [parseInt(c.slice(1,3),16),parseInt(c.slice(3,5),16),parseInt(c.slice(5,7),16)]}
function mix(a,b,f){f=Math.max(0,Math.min(1,f));return 'rgb('+a.map((v,i)=>Math.round(v+(b[i]-v)*f)).join(',')+')'}
function build(el, sc){
  el.innerHTML='';
  const spans=[];
  for(let y=0;y<sc.base.length;y++){
    const r=document.createElement('div');r.className='r';
    for(let x=0;x<sc.base[y].length;x++){
      const s=document.createElement('span');const c=sc.base[y][x];
      s.textContent=c[0];s.style.color=c[1];s.style.background=c[2];if(c[3])s.style.fontWeight=700;
      s._i=spans.length;r.appendChild(s);spans.push(s);
    }
    el.appendChild(r);
  }
  return spans;
}
function put(view,x,y,ch,fg,bg,bold){
  const s=view.spans[y*120+x];if(!s)return;
  if(!view.touched.has(s)){view.touched.add(s);}
  if(ch!==null)s.textContent=ch;
  if(fg)s.style.color=fg;
  if(bg)s.style.background=bg;
  s.style.fontWeight=bold?700:400;
}
function reset(view){
  for(const s of view.touched){const i=s._i;const c=view.sc.base[Math.floor(i/120)][i%120];
    s.textContent=c[0];s.style.color=c[1];s.style.background=c[2];s.style.fontWeight=c[3]?700:400;}
  view.touched.clear();
}
function bgOf(view,x,y){return hx(view.sc.base[y][x][2])}
function fgOf(view,x,y){return hx(view.sc.base[y][x][1])}
function label(view,t){
  const e=view.sc.exit;if(!e)return;
  const arrow={U:'▴',D:'▾',L:'◂',R:'▸'}[e.edge];
  const s=` ${arrow} Marlo · ${e.left} m `;
  let x=Math.min(Math.max(e.x-Math.floor(s.length/2),0),79-s.length), y=e.y+(e.edge==='U'?1:e.edge==='D'?-1:0);
  const pulse=0.65+0.35*Math.sin(t/30*Math.PI*2);
  for(let k=0;k<s.length;k++)put(view,x+k,y,s[k],k===1?mix([4,6,10],MAG,pulse):mix([4,6,10],[205,212,222],0.9),'#04060a',k===1);
}
const DRAW={
 A(view,t){
  const sc=view.sc,n=sc.total, breathe=0.50+0.12*Math.sin(t/60*Math.PI*2);
  const cycle=n+18, head=(t%cycle);
  for(const c of sc.cells){
    const d=head-c.i; let f=breathe, bold=false, col=CYAN;
    if(d>=0&&d<9){f=Math.max(breathe,1-d/9);}
    if(d===0){col=WHITE;f=1;bold=true}
    const bg=bgOf(view,c.x,c.y);
    if(c.ground)put(view,c.x,c.y,c.line,mix(bg,col,f),null,bold);
    else put(view,c.x,c.y,null,null,mix(bg,CYAN,f*0.35),false);
  }
  if(sc.target){
    const [tx,ty]=sc.target, p=0.5+0.5*Math.sin(t/30*Math.PI*2), flash=(head>=n&&head<n+6)?1:0;
    const g=new Map(sc.ground.map(([x,y,gr])=>[x+','+y,gr]));
    for(const [dx,dy,ch] of [[-1,-1,'┌'],[1,-1,'┐'],[-1,1,'└'],[1,1,'┘']]){
      const x=tx+dx,y=ty+dy, f=Math.max(p*0.8+0.2,flash);
      if(g.get(x+','+y))put(view,x,y,ch,mix(bgOf(view,x,y),MAG,f),null,true);
      else put(view,x,y,null,null,mix(bgOf(view,x,y),MAG,f*0.5),false);
    }
    put(view,tx,ty,null,null,mix(bgOf(view,tx,ty),MAG,0.25+0.35*Math.max(p,flash)),false);
  }
  label(view,t);
 },
 B(view,t){
  const sc=view.sc,n=sc.total,period=48;
  for(const c of sc.cells){
    let f=0.35, bold=false, col=CYAN;
    for(let w=0;w<3;w++){
      const head=((t*0.75)-w*period)%(n+period*1.0); const d=head-c.i;
      if(d>=0&&d<6){f=Math.max(f,1-d/6); if(d<1){col=WHITE;bold=true}}
    }
    const bg=bgOf(view,c.x,c.y);
    if(c.ground&&c.i%2===0)put(view,c.x,c.y,c.chev,mix(bg,col,f),null,bold);
    else if(!c.ground)put(view,c.x,c.y,null,null,mix(bg,CYAN,f*0.3),false);
  }
  if(sc.target){
    const [tx,ty]=sc.target,p=0.5+0.5*Math.sin(t/24*Math.PI*2);
    const g=new Map(sc.ground.map(([x,y,gr])=>[x+','+y,gr]));
    if(g.get((tx-1)+','+ty))put(view,tx-1,ty,'▕',mix(bgOf(view,tx-1,ty),MAG,0.3+0.7*p),null,false);
    if(g.get((tx+1)+','+ty))put(view,tx+1,ty,'▏',mix(bgOf(view,tx+1,ty),MAG,0.3+0.7*p),null,false);
  }
  label(view,t);
 },
 C(view,t){
  const sc=view.sc,n=sc.total,cycle=n+24,head=(t*0.9)%cycle, breathe=0.22+0.06*Math.sin(t/60*Math.PI*2);
  for(const c of sc.cells){
    const d=head-c.i; let f=breathe;
    if(d>=0&&d<12)f=Math.max(breathe,0.85*(1-d/12));
    const bg=bgOf(view,c.x,c.y);
    put(view,c.x,c.y,null,null,mix(bg,CYAN,f),false);
  }
  if(sc.target){
    const [tx,ty]=sc.target, r=(t/14)%4;
    for(const [x,y] of sc.ground){
      const dist=Math.abs(x-tx)+Math.abs(y-ty)*1.6;
      const ring=Math.max(0,1-Math.abs(dist-r*1.4)/1.1)*(1-r/4);
      const core=dist<0.5?0.35+0.2*Math.sin(t/15*Math.PI):0;
      const f=Math.max(ring*0.55,core);
      if(f>0.02)put(view,x,y,null,null,mix(bgOf(view,x,y),MAG,f),false);
    }
  }
  label(view,t);
 },
};
const views=[];
for(const el of document.querySelectorAll('.term[data-scene]')){
  const sc=SCENES[el.dataset.scene];
  views.push({el,sc,kind:el.dataset.kind,spans:build(el,sc),touched:new Set()});
}
let t=0, speed=1, acc=0, last=performance.now();
function frame(now){
  acc+=(now-last)*speed; last=now;
  while(acc>=1000/30){acc-=1000/30;t++}
  for(const v of views){reset(v);DRAW[v.kind](v,t)}
  document.getElementById('tick').textContent=t;
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
for(const b of document.querySelectorAll('button[data-speed]'))b.onclick=()=>{speed=+b.dataset.speed;
  for(const o of document.querySelectorAll('button[data-speed]'))o.classList.toggle('on',o===b)};
"""

PAGE_CSS = """
body{background:#0b0e14;color:#cdd4de;font-family:-apple-system,system-ui,sans-serif;margin:0;padding:24px}
h1{font-size:20px;margin:0 0 6px} h2{font-size:17px;margin:28px 0 4px}
p{margin:4px 0 10px;color:#a0aab9;max-width:1100px;line-height:1.45}
.row{display:flex;flex-wrap:wrap;gap:18px;align-items:flex-start}
.cap{font-size:12px;color:#7d8796;margin:4px 0 0}
.bar{position:sticky;top:0;background:#0b0e14;padding:8px 0;z-index:2;font-size:13px;color:#7d8796}
button{background:#141a24;color:#cdd4de;border:1px solid #263142;border-radius:4px;padding:3px 10px;margin-left:6px;cursor:pointer}
button.on{border-color:#00e5ff;color:#00e5ff}
.term span{transition:none}
"""


def main(out):
    data = json.load(open(os.path.join(S, "tmp_track.json")))
    scenes = {name: scene(name, data[name]) for name in ("near", "far")}
    body = [
        "<h1>Job tracking · round 1</h1>",
        "<p>Track a job from JOBS and the walk to its target is drawn over the street, "
        "from your tile, by A* over walkable tiles, redrawn as you move. Each concept is "
        "shown with the target in view (Marlo's body, 30 m) and off the map (95 m: the "
        "line runs to the edge, a tag there names the target and what's left). The JOBS "
        "pane gains a <code>⌖ tracking</code> line. Animated at the game's 30 ticks/s.</p>",
        '<div class="bar">speed <button data-speed="1" class="on">1×</button>'
        '<button data-speed="0.25">¼×</button><button data-speed="0">pause</button>'
        ' · tick <span id="tick">0</span></div>',
    ]
    for key, name, desc in CONCEPTS:
        body.append(f"<h2>{key} · {name}</h2><p>{desc}</p><div class='row'>")
        for sc, cap in (("near", "target in view"), ("far", "target off the map")):
            body.append(f"<div><div class='term' data-scene='{sc}' data-kind='{key}' "
                        f"style='font-size:12.5px'></div><div class='cap'>{key} · {cap}</div></div>")
        body.append("</div>")
    html = ("<!DOCTYPE html><html><head><meta charset='utf-8'><title>Job tracking</title>"
            f"<style>{CSS}{PAGE_CSS}</style></head><body>{''.join(body)}"
            f"<script>const SCENES={json.dumps(scenes)};{JS}</script></body></html>")
    open(out, "w").write(html)


if __name__ == "__main__":
    main(sys.argv[1])
