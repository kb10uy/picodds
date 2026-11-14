import os

import FreeCAD as App
import Part
from FreeCAD import Vector as V

OUT_DIR = globals().get("OUT_DIR") or os.path.dirname(os.path.abspath(__file__))

BOARD_R = 25.4
BOARD_FLAT_X = 49.25 / 2
NOTCH_X = 48.75 / 2
NOTCH_R = 1.6
BOARD_T = 1.6

GAP = 0.4
WALL = 2.5
FLOOR = 2.0
UNDER = 3.0
HEADROOM = 12.5
LID_T = 2.0

IN_R = BOARD_R + GAP
OUT_R = IN_R + WALL
Z_BOARD = FLOOR + UNDER
Z_BOARD_TOP = Z_BOARD + BOARD_T
Z_CEIL = Z_BOARD_TOP + HEADROOM
Z_TOP = Z_CEIL + LID_T

BOSS_CLIP_X = 21.8
BASE_BOSS_R = 3.5
LID_BOSS_R = 3.0
LID_BOSS_CLEAR = 0.3
CLEAR_HOLE_R = 1.7
PILOT_R = 1.25
PILOT_DEPTH = 9.0
CBORE_R = 3.1

LIP_T = 0.8
LIP_H = 2.0
LIP_CLEAR = 0.2

USB_X = 132.08 - 127.0
USB_Z = Z_BOARD_TOP + 2.5 + 1.0 + 1.6
USB_W = 13.0
USB_H = 8.0

J1_X = 147.32 - 127.0
J1_Y = -(110.49 - 101.6)
J1_INSET = 4.0
J1_L = 9.0


def cyl(r, h, x=0.0, y=0.0, z=0.0):
    return Part.makeCylinder(r, h, V(x, y, z))


def box(x0, y0, z0, x1, y1, z1):
    return Part.makeBox(x1 - x0, y1 - y0, z1 - z0, V(x0, y0, z0))


def slot_xz(w, h, cx, cz, y0, y1):
    r = min(w, h) / 2
    core = box(cx - w / 2 + r, y0, cz - h / 2, cx + w / 2 - r, y1, cz + h / 2)
    ends = [
        Part.makeCylinder(r, y1 - y0, V(cx + s * (w / 2 - r), y0, cz), V(0, 1, 0))
        for s in (-1, 1)
    ]
    return core.fuse(ends)


def slot_xy(w, l, cx, cy, z0, z1):
    r = min(w, l) / 2
    core = box(cx - w / 2, cy - l / 2 + r, z0, cx + w / 2, cy + l / 2 - r, z1)
    ends = [cyl(r, z1 - z0, cx, cy + s * (l / 2 - r), z0) for s in (-1, 1)]
    return core.fuse(ends)


def side_clip(sign, z0, z1):
    big = OUT_R + 5
    if sign > 0:
        return box(BOSS_CLIP_X, -big, z0, big, big, z1)
    return box(-big, -big, z0, -BOSS_CLIP_X, big, z1)


def make_base():
    shell = cyl(OUT_R, Z_CEIL).cut(cyl(IN_R, Z_CEIL - FLOOR, z=FLOOR))
    for s in (-1, 1):
        x = s * NOTCH_X
        boss = cyl(BASE_BOSS_R, UNDER, x, 0, FLOOR)
        bridge = box(min(x, s * IN_R), -BASE_BOSS_R, FLOOR, max(x, s * IN_R), BASE_BOSS_R, Z_BOARD)
        boss = boss.fuse(bridge).common(side_clip(s, FLOOR, Z_BOARD))
        shell = shell.fuse(boss)
    for s in (-1, 1):
        x = s * NOTCH_X
        shell = shell.cut(cyl(CLEAR_HOLE_R, Z_BOARD + 1, x, 0, -0.5))
        shell = shell.cut(cyl(CBORE_R, FLOOR + 0.5, x, 0, -0.5))
    usb = slot_xz(USB_W, USB_H, USB_X, USB_Z, 0, OUT_R + 2)
    shell = shell.cut(usb)
    shell = shell.cut(box(J1_X - J1_INSET, J1_Y - J1_L / 2, Z_BOARD_TOP, OUT_R + 1, J1_Y + J1_L / 2, Z_CEIL + 1))
    return shell.removeSplitter()


def make_lid():
    lid = cyl(OUT_R, LID_T, z=Z_CEIL)
    lip_out = IN_R - LIP_CLEAR
    lip = cyl(lip_out, LIP_H, z=Z_CEIL - LIP_H).cut(cyl(lip_out - LIP_T, LIP_H, z=Z_CEIL - LIP_H))
    lid = lid.fuse(lip)
    boss_h = Z_CEIL - Z_BOARD_TOP
    for s in (-1, 1):
        x = s * NOTCH_X
        boss = cyl(LID_BOSS_R, boss_h, x, 0, Z_BOARD_TOP)
        boss = boss.common(cyl(IN_R - LID_BOSS_CLEAR, boss_h, z=Z_BOARD_TOP))
        boss = boss.common(side_clip(s, Z_BOARD_TOP, Z_CEIL))
        lid = lid.fuse(boss)
    for s in (-1, 1):
        lid = lid.cut(cyl(PILOT_R, PILOT_DEPTH, s * NOTCH_X, 0, Z_BOARD_TOP - 0.5))
    lid = lid.cut(box(J1_X - J1_INSET, J1_Y - J1_L / 2, Z_CEIL - LIP_H - 1, OUT_R + 1, J1_Y + J1_L / 2, Z_TOP + 1))
    return lid.removeSplitter()


def make_board():
    disc = cyl(BOARD_R, BOARD_T, z=Z_BOARD)
    disc = disc.common(box(-BOARD_FLAT_X, -BOARD_R, Z_BOARD, BOARD_FLAT_X, BOARD_R, Z_BOARD_TOP))
    for s in (-1, 1):
        disc = disc.cut(cyl(NOTCH_R, BOARD_T + 1, s * NOTCH_X, 0, Z_BOARD - 0.5))
    return disc


def kc(x, y):
    return x - 127.0, -(y - 101.6)


def make_parts():
    z = Z_BOARD_TOP
    parts = {}
    x0, y0 = kc(123.18, 80.94)
    x1, y1 = kc(140.98, 101.94)
    parts["XIAO_RP2040"] = box(x0, y1, z + 2.5, x1, y0, z + 3.5).fuse(
        box(x0, y1, z, x1, y0, z + 2.5).cut(box(x0 + 2.5, y1, z, x1 - 2.5, y0, z + 2.5))
    )
    ux0, uy0 = kc(127.58, 79.44)
    ux1, uy1 = kc(136.58, 83.0)
    parts["USB_C"] = box(ux0, uy1, z + 3.5, ux1, uy0, z + 6.7)
    cx, cy = kc(142.24, 115.59)
    parts["C2_5x11"] = cyl(2.5, 12.0, cx, cy, z)
    jx, jy = kc(147.32, 110.49)
    parts["J1_header"] = box(jx - 1.27, jy - 2.54, z, jx + 1.27, jy + 2.54, z + 2.5).fuse(
        [box(jx - 0.32, jy + d - 0.32, z, jx + 0.32, jy + d + 0.32, z + 8.5) for d in (-1.27, 1.27)]
    )
    for name, (kx, ky, w, l) in {
        "U2_DIP14": (115.57, 93.98, 6.6, 19.0),
        "U3_DIP8": (125.73, 120.65, 9.8, 6.6),
    }.items():
        px, py = kc(kx, ky)
        parts[name] = box(px - w / 2, py - l / 2, z, px + w / 2, py + l / 2, z + 4.5)
    dx, dy = kc(146.03, 101.6)
    parts["C4_disc"] = box(dx - 2.5, dy - 1.25, z, dx + 2.5, dy + 1.25, z + 8.0)
    return parts


def build():
    path = os.path.join(OUT_DIR, "picodds_case.FCStd")
    doc = App.newDocument("picodds_case")
    base = make_base()
    lid = make_lid()
    board = make_board()
    parts = make_parts()

    def add(name, shape, color=None):
        obj = doc.addObject("Part::Feature", name)
        obj.Shape = shape
        if color and App.GuiUp:
            obj.ViewObject.ShapeColor = color
        return obj

    add("Base", base)
    add("Lid", lid)
    ref = doc.addObject("App::DocumentObjectGroup", "Reference")
    ref.addObject(add("Board", board))
    for name, shape in parts.items():
        ref.addObject(add(name, shape))
    doc.recompute()
    doc.saveAs(path)

    Part.export([doc.getObject("Base")], os.path.join(OUT_DIR, "picodds_case_base.step"))
    Part.export([doc.getObject("Lid")], os.path.join(OUT_DIR, "picodds_case_lid.step"))
    import Mesh
    import MeshPart

    for name, shape in (("base", base), ("lid", lid)):
        mesh = MeshPart.meshFromShape(Shape=shape, LinearDeflection=0.02, AngularDeflection=0.2)
        mesh.write(os.path.join(OUT_DIR, f"picodds_case_{name}.stl"))
    return doc, base, lid, board, parts


if __name__ == "__main__" or globals().get("RUN_BUILD"):
    build()
