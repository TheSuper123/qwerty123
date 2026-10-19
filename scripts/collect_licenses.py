"""Collect exact dependency source notices without traversing the project data directory."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument("destination", type=Path)
args = parser.parse_args()
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1"], text=True))
args.destination.mkdir(parents=True, exist_ok=True)
records = []
combined = [(Path(__file__).resolve().parents[1] / "LICENSE").read_text(encoding="utf-8"), "\nTHIRD-PARTY DEPENDENCY LICENSES\n"]
for package in metadata["packages"]:
    if package["source"] is None:
        continue
    root = Path(package["manifest_path"]).parent
    destination = args.destination / (package["name"] + "-" + package["version"])
    destination.mkdir(exist_ok=True)
    copied = []
    for path in root.rglob("*"):
        if not path.is_file() or path.is_symlink() or path.stat().st_size > 2_000_000:
            continue
        if not path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")):
            continue
        target = destination / path.relative_to(root)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, target)
        copied.append(str(path.relative_to(root)))
    record = {"name": package["name"], "version": package["version"], "license": package["license"],
              "authors": package["authors"], "repository": package["repository"], "notice_files": copied,
              "source": "https://crates.io/api/v1/crates/" + package["name"] + "/" + package["version"] + "/download"}
    (destination / "SOURCE.json").write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    records.append(record)
    combined.append("\n" + package["name"] + " " + package["version"] + "\n" + json.dumps(record, ensure_ascii=False, indent=2) + "\n")
    for relative in copied:
        combined.append("\n--- " + relative + " ---\n" + (root / relative).read_text(encoding="utf-8", errors="replace"))
(args.destination / "DEPENDENCIES.json").write_text(json.dumps(records, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
(args.destination / "ALL_LICENSES.txt").write_text("\n".join(combined), encoding="utf-8")
print(f"Collected notices for {len(records)} exact dependency versions.")
