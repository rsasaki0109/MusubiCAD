"""Render docs/assets/agent-demo/conversation.gif from a recorded agent session.

The GIF abridges docs/assets/agent-demo/session.json, a real Claude Code
session with the MusubiCAD plugin. Every quoted fact in the frames is checked
against the transcript before anything is drawn, so the animation cannot claim
a number the session did not produce.

    python docs/assets/generate-agent-demo.py   # needs Pillow
"""

from __future__ import annotations

import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent
SESSION = ROOT / "agent-demo" / "session.json"
OUTPUT = ROOT / "agent-demo" / "conversation.gif"
FONT = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"
BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"

WIDTH, HEIGHT, MARGIN, LINE, GAP = 960, 600, 28, 25, 14
BACKGROUND = (17, 23, 34)
COLORS = {
    "user": (109, 213, 255),
    "tool": (154, 163, 178),
    "ok": (94, 214, 141),
    "bad": (255, 120, 112),
    "text": (231, 237, 247),
    "dim": (120, 130, 148),
}

# (kind, text, facts that must appear in the transcript)
SCENES = [
    [
        ("user", "> Make the bearing bore 8 mm. Show me what changes before you apply it.", []),
        ("tool", "  musubicad · patch_dry_run", ["patch_dry_run"]),
        ("ok", "  ✓ verification passed: regenerates, expected mass increase holds", ['"passed": true']),
        ("text", "bore_diameter  18 mm → 8 mm", ["18 mm → 8 mm"]),
        ("text", "mass           156.00 g → 163.72 g (+7.72 g)", ["156.00 g → 163.72 g (+7.72 g)"]),
        ("text", "overall size   unchanged, 96 × 72 × 14 mm", ["96 × 72 × 14 mm"]),
        ("text", "Should I apply it?", ["Should I apply it?"]),
    ],
    [
        ("user", "> Yes, apply it.", []),
        ("tool", "  musubicad · patch_apply", ["patch_apply"]),
        ("ok", "  ✓ re-verified, then written", ["checked again before it was written"]),
    ],
    [
        ("user", "> Actually, make the bore 200 mm.", []),
        ("tool", "  musubicad · patch_dry_run", ["Enlarge the bearing bore to 200 mm"]),
        ("bad", "  ✗ feature 'feature:bearing_bore' leaves no solid", ["feature 'feature:bearing_bore'", "got 0"]),
        ("text", "I didn't apply the 200 mm bore. The file hasn't been changed.", ["I didn't apply the 200 mm bore", "the file hasn't been changed"]),
        ("text", "The whole plate is only 96 × 72 mm.", ["the whole plate is only 96 × 72 mm"]),
        ("text", "Did you mean 20 mm? That would leave a 9 mm wall.", ["Did you mean 20 mm?", "9 mm wall"]),
    ],
    [
        ("user", "> Keep it at 8 mm. Export it as STEP and STL.", []),
        ("tool", "  musubicad · export_document", ["export_document"]),
        ("ok", "  ✓ bearing_carrier.step  44.6 KB, millimetres", ['"bytes": 44629', "44.6 KB"]),
        ("ok", "  ✓ bearing_carrier.stl   1,592 triangles", ['"triangles": 1592', "1,592 triangles"]),
    ],
]


def verify_facts(transcript: str) -> None:
    missing = [
        fact
        for scene in SCENES
        for _, _, facts in scene
        for fact in facts
        if fact not in transcript
    ]
    if missing:
        raise SystemExit(f"facts not found in {SESSION.name}: {missing}")


def render() -> list[Image.Image]:
    font = ImageFont.truetype(FONT, 17)
    bold = ImageFont.truetype(BOLD, 17)
    frames: list[Image.Image] = []
    durations: list[int] = []

    def height_of(lines: list[tuple[str, str]]) -> int:
        return sum(LINE + (GAP if kind == "user" else 0) for kind, _ in lines)

    def frame(lines: list[tuple[str, str]], hold_ms: int) -> None:
        # A chat that scrolls: drop the oldest lines once the screen is full.
        while height_of(lines) > HEIGHT - 70:
            lines = lines[1:]
        image = Image.new("RGB", (WIDTH, HEIGHT), BACKGROUND)
        draw = ImageDraw.Draw(image)
        draw.text((MARGIN, 14), "Claude Code + MusubiCAD plugin  ·  abridged real session", font=font, fill=COLORS["dim"])
        y = 52
        for kind, text in lines:
            if kind == "user":
                y += GAP
            draw.text((MARGIN, y), text, font=bold if kind == "user" else font, fill=COLORS[kind])
            y += LINE
        frames.append(image)
        durations.append(hold_ms)

    shown: list[tuple[str, str]] = []
    for scene in SCENES:
        for kind, text, _ in scene:
            if kind == "user":
                for end in range(8, len(text) + 8, 8):
                    frame(shown + [(kind, text[:end])], 45)
                shown.append((kind, text))
                frame(shown, 500)
            else:
                shown.append((kind, text))
                frame(shown, 650 if kind != "text" else 450)
        frame(shown, 2400)
    frames[0].info["durations"] = durations
    return frames


def main() -> None:
    transcript = SESSION.read_text(encoding="utf-8")
    # Facts may sit inside JSON-escaped strings; compare against the decoded text too.
    decoded = json.dumps(json.loads(transcript), ensure_ascii=False)
    verify_facts(transcript + "\n" + decoded + "\n" + decoded.replace('\\"', '"'))
    frames = render()
    durations = frames[0].info.pop("durations")
    palette = [frame.quantize(colors=32, method=Image.Quantize.MEDIANCUT) for frame in frames]
    palette[0].save(
        OUTPUT, save_all=True, append_images=palette[1:], duration=durations, loop=0, optimize=True
    )
    print(f"{OUTPUT.relative_to(ROOT.parent.parent)}: {len(frames)} frames, {OUTPUT.stat().st_size} bytes")


if __name__ == "__main__":
    main()
