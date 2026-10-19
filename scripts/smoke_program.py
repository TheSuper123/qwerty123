"""Exercise the compiled process, authenticated HTTP, instance lock and clean shutdown."""
import argparse
import json
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=Path)
args = parser.parse_args()
binary = str(args.binary.resolve())
with tempfile.TemporaryDirectory(prefix="most-native-smoke-") as folder:
    command = [binary, "--no-browser", "--port", "0", "--data-dir", folder]
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
    lines = queue.Queue()
    def capture():
        for line in process.stdout:
            lines.put(line.strip())
    threading.Thread(target=capture, daemon=True).start()
    try:
        deadline = time.monotonic() + 20
        access = None
        while time.monotonic() < deadline:
            try:
                line = lines.get(timeout=0.5)
            except queue.Empty:
                if process.poll() is not None:
                    raise AssertionError("Program exited before HTTP readiness.")
                continue
            if line.startswith("http://127.0.0.1:") and "/#token=" in line:
                access = line
                break
        assert access, "No local access URL within readiness deadline."
        base, token = access.split("/#token=", 1)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        request = urllib.request.Request(base + "/api/state", headers={"Authorization": "Bearer " + token})
        with opener.open(request, timeout=10) as response:
            snapshot = json.load(response)
        assert snapshot["artifacts"] == [] and snapshot["jobs"] == []
        assert snapshot["capabilities"]["remote"] is False
        duplicate = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            duplicate.communicate(timeout=10)
            assert duplicate.returncode != 0, "A second process acquired the same data directory."
        finally:
            if duplicate.poll() is None:
                duplicate.terminate()
                duplicate.wait(timeout=10)
        with opener.open(urllib.request.Request(base + "/api/diagnostics", headers={"Authorization": "Bearer " + token}), timeout=10) as response:
            diagnostics = json.load(response)
        assert diagnostics["hardware_validation"] == "not_run"
        assert token not in json.dumps(diagnostics)
        shutdown = urllib.request.Request(base + "/api/shutdown", headers={"Authorization": "Bearer " + token}, method="POST")
        with opener.open(shutdown, timeout=10) as response:
            assert response.status == 204
        process.wait(timeout=15)
        assert process.returncode == 0
        print("Compiled process: authenticated API, protected data directory, diagnostics and graceful shutdown: PASS (no physical device test).")
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=10)
