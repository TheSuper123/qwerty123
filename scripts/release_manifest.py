"""A download becomes visible only after native CI builds and release upload succeed."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--assets", type=Path, required=True)
parser.add_argument("--repository", required=True)
parser.add_argument("--commit", required=True)
parser.add_argument("--output", type=Path, default=Path("site/downloads.json"))
args = parser.parse_args()
metadata = json.loads((args.assets / "windows-build.json").read_text())
version = metadata["version"]
base = f"https://github.com/{args.repository}/releases/download/v{version}/"
files = {}
for name in ["MostServer-windows-x64.zip", "MostServer.exe", "MostStore-unsigned.ipa"]:
    path = args.assets / name
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if name in metadata["sha256"] and metadata["sha256"][name] != digest:
        raise SystemExit("Downloaded build artifact checksum mismatch: " + name)
    files[name] = {"url": base + name, "sha256": digest, "bytes": path.stat().st_size}
result = {"schema": 1, "status": "ready", "version": version, "commit": args.commit,
          "published_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
          "windows_code_signed": False, "ios_requires_signing": True,
          "hardware_validation": "not_run", "files": files}
args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
if args.output.parent.name == "site":
    (args.output.parent / "build-status.json").write_text(json.dumps({"status": "success", "commit": args.commit, "version": version}, indent=2) + "\n", encoding="utf-8")
print("Release download manifest generated from verified build artifacts.")
