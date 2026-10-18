# Waits for the OpenSK device and runs a CTAP2 makeCredential, which requires a touch.
import os, sys, time
from fido2.hid import CtapHidDevice
from fido2.ctap2 import Ctap2

print("waiting for an OpenSK device...", flush=True)
while not (devs := list(CtapHidDevice.list_devices())):
    time.sleep(0.5)
ctap = Ctap2(devs[0])
print(">>> TOUCH THE BUTTON NOW (LED blinks green; 30 s timeout)", flush=True)
t = time.time()
try:
    att = ctap.make_credential(
        os.urandom(32),
        {"id": "test.local", "name": "test"},
        {"id": b"u1", "name": "u"},
        [{"type": "public-key", "alg": -7}],
    )
    print(f"OK: touch recognized after {time.time()-t:.1f}s, fmt={att.fmt}", flush=True)
except Exception as e:
    print(f"FAILED after {time.time()-t:.1f}s: {e!r}", flush=True)
    sys.exit(1)
