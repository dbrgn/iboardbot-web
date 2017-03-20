from typing import Sequence, Tuple
from cffi import FFI

ffi = FFI()
lib = ffi.dlopen('libsvg2polylines.so')

CoordinatePair = Tuple[float, float]
Polyline = Sequence[CoordinatePair]
Polylines = Sequence[Polyline]


with open('svg2polylines.h', 'r') as f:
    ffi.cdef(f.read())


def parse(svg_data: str) -> Polylines:
    """
    Parse SVG data, return list of polylines.
    """
    polylines = ffi.new('Polyline**')
    polylines_len = ffi.new('size_t*')
    lib.svg_str_to_polylines(svg_data, polylines, polylines_len)
    out = []
    for i in range(polylines_len[0]):
        p = polylines[0][i]
        tmp = []
        for j in range(p.len):
            tmp.append((p.ptr[j].x, p.ptr[j].y))
        out.append(tmp)
    return out
