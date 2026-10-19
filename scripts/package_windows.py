"""Package an actual Windows PE build; never relabel a Linux executable as .exe."""
from pathlib import Path
import hashlib
import json
import shutil
import struct
import subprocess
import tomllib
import zipfile

root = Path(__file__).resolve().parents[1]
version = tomllib.loads((root / "apps/server/Cargo.toml").read_text())["package"]["version"]
binary = root / "target/release/MostServer.exe"
data = binary.read_bytes()
pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
if data[:2] != b"MZ" or data[pe_offset:][:4] != b"PE\0\0" or struct.unpack_from("<H", data, pe_offset + 4)[0] != 0x8664:
    raise SystemExit("The build is not a Windows x64 PE executable.")
subprocess.run([str(binary), "--self-test"], check=True)
if "isideload" not in subprocess.check_output([str(binary), "--licenses"], text=True, encoding="utf-8"):
    raise SystemExit("The executable is missing embedded dependency notices.")
dist = root / "dist"
staging = dist / "MostServer"
staging.mkdir(parents=True, exist_ok=True)
shutil.copyfile(binary, staging / "MostServer.exe")
for source in ["LICENSE", "THIRD_PARTY_NOTICES.md", "docs/PROGRAM_WINDOWS.md"]:
    shutil.copyfile(root / source, staging / Path(source).name)
(staging / "VERSION.txt").write_text(version + "\n", encoding="utf-8")
shutil.copytree(dist / "notices", staging / "third-party-licenses", dirs_exist_ok=True)
archive = dist / "MostServer-windows-x64.zip"
with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=6) as output:
    for path in sorted(staging.rglob("*")):
        if path.is_file():
            output.write(path, path.relative_to(dist))
shutil.copyfile(binary, dist / "MostServer.exe")
checksums = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in [archive, dist / "MostServer.exe"]}
(dist / "SHA256SUMS.txt").write_text("".join(digest + "  " + name + "\n" for name, digest in checksums.items()), encoding="ascii")
(dist / "windows-build.json").write_text(json.dumps({"version": version, "sha256": checksums, "bytes": archive.stat().st_size}, indent=2) + "\n", encoding="utf-8")
print("Windows EXE and ZIP packaged; PE header and self-test verified.")
