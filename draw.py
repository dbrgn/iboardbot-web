from io import BytesIO
from PIL import Image, ImageDraw

from svg2polylines import Polylines


BG_COLOR = '#AED389'
WIDTH = 400
HEIGHT = 150
SCALE = 2


def get_png(data: Polylines) -> BytesIO:
    img = Image.new('RGBA', (WIDTH * SCALE, HEIGHT * SCALE), BG_COLOR)
    draw = ImageDraw.Draw(img)
    for polyline in data:
        print('Drawing polyline:')
        last = None
        for x, y in polyline:
            if last is not None:
                print('  (%f,%f) -> (%f, %f)' % (last[0], last[1], x, y))
                draw.line((last[0], last[1], x, y), fill='black', width=SCALE)
            last = (x, y)
    f = BytesIO()
    img.save(f, format='png')
    return f
