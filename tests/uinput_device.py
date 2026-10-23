"""A virtual absolute pointer and keyboard through /dev/uinput, for live tests of what a
compositor delivers (0.5.7, #198). It creates a new device; the real ones are untouched,
but the desktop's pointer does move, so the tests that use it run only with
`TRE_LIVE_INPUT=1` and write access to /dev/uinput."""
import fcntl, os, struct, time

EV_SYN, EV_KEY, EV_REL, EV_ABS = 0, 1, 2, 3
BTN_LEFT, BTN_RIGHT = 0x110, 0x111
REL_WHEEL = 8
ABS_X, ABS_Y = 0, 1
UI_SET_EVBIT, UI_SET_KEYBIT, UI_SET_RELBIT, UI_SET_ABSBIT = 0x40045564, 0x40045565, 0x40045566, 0x40045567
UI_DEV_CREATE, UI_DEV_DESTROY, UI_ABS_SETUP, UI_DEV_SETUP = 0x5501, 0x5502, 0x401C5504, 0x405C5503
ABS_MAX = 65535

# Linux key codes for the characters the spike types.
KEYS = {c: k for c, k in zip("qwertyuiop", range(16, 26))}
KEYS.update({c: k for c, k in zip("asdfghjkl", range(30, 39))})
KEYS.update({c: k for c, k in zip("zxcvbnm", range(44, 51))})
KEYS.update({" ": 57, "\n": 28, "\b": 14})


class Device:
    def __init__(self, width, height):
        self.width, self.height = width, height
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        for ev in (EV_KEY, EV_REL, EV_ABS, EV_SYN):
            fcntl.ioctl(self.fd, UI_SET_EVBIT, ev)
        for key in [BTN_LEFT, BTN_RIGHT, *KEYS.values()]:
            fcntl.ioctl(self.fd, UI_SET_KEYBIT, key)
        fcntl.ioctl(self.fd, UI_SET_RELBIT, REL_WHEEL)
        for axis in (ABS_X, ABS_Y):
            fcntl.ioctl(self.fd, UI_SET_ABSBIT, axis)
            # struct uinput_abs_setup { u16 code; struct input_absinfo { s32 value, minimum, maximum, fuzz, flat, resolution } }
            fcntl.ioctl(self.fd, UI_ABS_SETUP, struct.pack("H2x6i", axis, 0, 0, ABS_MAX, 0, 0, 0))
        name = b"tre-spike virtual input".ljust(80, b"\0")
        fcntl.ioctl(self.fd, UI_DEV_SETUP, struct.pack("HHHH80sI", 0x03, 0x1234, 0x5678, 1, name, 0))
        fcntl.ioctl(self.fd, UI_DEV_CREATE)
        time.sleep(1.0)  # let the compositor pick the device up

    def emit(self, kind, code, value):
        os.write(self.fd, struct.pack("llHHi", 0, 0, kind, code, value))

    def sync(self):
        self.emit(EV_SYN, 0, 0)

    def move(self, x, y):
        """Move to global desktop coordinates (x, y)."""
        self.emit(EV_ABS, ABS_X, round(x / (self.width - 1) * ABS_MAX))
        self.emit(EV_ABS, ABS_Y, round(y / (self.height - 1) * ABS_MAX))
        self.sync()
        time.sleep(0.15)

    def click(self, button=BTN_LEFT):
        self.emit(EV_KEY, button, 1); self.sync(); time.sleep(0.08)
        self.emit(EV_KEY, button, 0); self.sync(); time.sleep(0.15)

    def scroll(self, notches):
        self.emit(EV_REL, REL_WHEEL, notches); self.sync(); time.sleep(0.15)

    def type(self, text):
        for ch in text:
            code = KEYS[ch]
            self.emit(EV_KEY, code, 1); self.sync(); time.sleep(0.04)
            self.emit(EV_KEY, code, 0); self.sync(); time.sleep(0.06)

    def close(self):
        fcntl.ioctl(self.fd, UI_DEV_DESTROY)
        os.close(self.fd)
