"""Compares each pair of screenshots capture.js took (NAME.ruby.png and
NAME.rust.png): same size, and how many pixels differ. Writes a JSON
report, and a diff image (differing pixels in red over a faded copy) for
any pair that is not identical.

Usage: python3 -I compare.py SHOTS_DIR
"""

import json
import sys
from pathlib import Path

from PIL import Image, ImageChops

shots = Path(sys.argv[1])
report = {}
for ruby in sorted(shots.glob("*.ruby.png")):
    name = ruby.name.removesuffix(".ruby.png")
    a = Image.open(ruby).convert("RGB")
    b = Image.open(shots / f"{name}.rust.png").convert("RGB")
    entry = {"ruby_size": a.size, "rust_size": b.size}
    if a.size == b.size:
        diff = ImageChops.difference(a, b).convert("L").point(lambda v: 255 if v else 0)
        differing = sum(1 for v in diff.get_flattened_data() if v)
        entry["differing_pixels"] = differing
        entry["total_pixels"] = a.size[0] * a.size[1]
        if differing:
            faded = Image.blend(a, Image.new("RGB", a.size, "white"), 0.7)
            faded.paste(Image.new("RGB", a.size, (220, 0, 0)), mask=diff)
            faded.save(shots / f"{name}.diff.png")
            entry["diff_box"] = diff.getbbox()
    report[name] = entry
    print(name, entry.get("differing_pixels", "size differs"), entry)
(shots / "report.json").write_text(json.dumps(report, indent=2))

# Recompressed losslessly, to keep the committed screenshots light
for png in shots.glob("*.png"):
    image = Image.open(png)
    image.load()
    image.save(png, optimize=True)
