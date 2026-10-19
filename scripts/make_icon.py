"""Generate the project's geometric M icon as an opaque 1024px PNG, without dependencies."""
from pathlib import Path
import struct
import zlib

size = 1024
polygon = [(8, 32), (8, 15), (14, 15), (24, 25), (34, 15), (40, 15), (40, 32), (34, 32), (34, 24), (24, 34), (14, 24), (14, 32)]
background = bytes((16, 22, 18))
foreground = bytes((209, 245, 139))
scanlines = bytearray()
for row in range(size):
    y = (row + 0.5) * 48 / size
    crossings = []
    for index, (x1, y1) in enumerate(polygon):
        x2, y2 = polygon[(index + 1) % len(polygon)]
        if (y1 <= y < y2) or (y2 <= y < y1):
            crossings.append((x1 + (y - y1) * (x2 - x1) / (y2 - y1)) * size / 48)
    crossings.sort()
    pixels = bytearray(background * size)
    for left, right in zip(crossings[::2], crossings[1::2]):
        start, end = max(0, int(left)), min(size, int(right))
        pixels[start * 3:end * 3] = foreground * (end - start)
    scanlines.extend(b"\0" + pixels)
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(scanlines, 9)) + chunk(b"IEND", b"")
path = Path(__file__).resolve().parents[1] / "apps/ios/Assets.xcassets/AppIcon.appiconset/MostIcon.png"
path.parent.mkdir(parents=True, exist_ok=True)
path.write_bytes(png)
print("Generated opaque Most app icon.")
