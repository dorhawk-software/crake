"""Save an X11 window's own pixels (XGetImage): no compositor, no flash.
usage: xwin.py <window id hex> <out.png>"""
import ctypes, sys
from PIL import Image
x11 = ctypes.CDLL("libX11.so.6")
class XWindowAttributes(ctypes.Structure):
    _fields_ = [("x", ctypes.c_int), ("y", ctypes.c_int), ("width", ctypes.c_int), ("height", ctypes.c_int),
                ("border_width", ctypes.c_int), ("depth", ctypes.c_int), ("visual", ctypes.c_void_p),
                ("root", ctypes.c_ulong), ("class", ctypes.c_int), ("bit_gravity", ctypes.c_int),
                ("win_gravity", ctypes.c_int), ("backing_store", ctypes.c_int), ("backing_planes", ctypes.c_ulong),
                ("backing_pixel", ctypes.c_ulong), ("save_under", ctypes.c_int), ("colormap", ctypes.c_ulong),
                ("map_installed", ctypes.c_int), ("map_state", ctypes.c_int), ("all_event_masks", ctypes.c_long),
                ("your_event_mask", ctypes.c_long), ("do_not_propagate_mask", ctypes.c_long),
                ("override_redirect", ctypes.c_int), ("screen", ctypes.c_void_p)]
class XImage(ctypes.Structure):
    _fields_ = [("width", ctypes.c_int), ("height", ctypes.c_int), ("xoffset", ctypes.c_int), ("format", ctypes.c_int),
                ("data", ctypes.c_void_p), ("byte_order", ctypes.c_int), ("bitmap_unit", ctypes.c_int),
                ("bitmap_bit_order", ctypes.c_int), ("bitmap_pad", ctypes.c_int), ("depth", ctypes.c_int),
                ("bytes_per_line", ctypes.c_int), ("bits_per_pixel", ctypes.c_int)]
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XGetImage.restype = ctypes.POINTER(XImage)
x11.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
d = x11.XOpenDisplay(None)
win = int(sys.argv[1], 16)
attrs = XWindowAttributes()
x11.XGetWindowAttributes(ctypes.c_void_p(d), ctypes.c_ulong(win), ctypes.byref(attrs))
img = x11.XGetImage(d, win, 0, 0, attrs.width, attrs.height, ctypes.c_ulong(0xFFFFFFFF), 2)
if not img:
    sys.exit("XGetImage failed")
im = img.contents
buf = ctypes.string_at(im.data, im.bytes_per_line * im.height)
Image.frombuffer("RGBA", (im.width, im.height), buf, "raw", "BGRA", im.bytes_per_line, 1).convert("RGB").save(sys.argv[2])
print(im.width, im.height, im.depth)
