import os
import base64

from flask import Flask, request, redirect, flash, render_template

import svg2polylines
import draw


app = Flask(__name__)

app.static_url_path = '/static'
app.static_folder = 'static'

app.config['MAX_CONTENT_LENGTH'] = 16 * 1024 * 1024
if 'SECRET_KEY' in os.environ:
    app.secret_key = os.environ['SECRET_KEY']
else:
    raise ValueError('Please set SECRET_KEY env var!')


def allowed_file(filename):
    return '.' in filename and \
            filename.rsplit('.', 1)[1].lower() == 'svg'


@app.route('/', methods=['GET', 'POST'])
def hello():
    if request.method != 'POST':
        return render_template('index.html')
    if 'file' not in request.files:
        flash('No file uploaded')
        return redirect('/')
    svg = request.files['file']
    if svg.filename == '':
        flash('No file uploaded')
        return redirect('/')
    if not allowed_file(svg.filename):
        flash('Only .svg files are allowed')
        return redirect('/')
    if svg:
        svg_data = svg.read()
        scale_factor = 1
        if request.form.get('scale'):
            try:
                scale_percent = int(request.form.get('scale'))
                scale_factor = scale_percent / 100
            except ValueError:
                pass
        data = svg2polylines.parse(svg_data)
        image = draw.get_png(data, scale_factor=scale_factor)
        image_url = 'data:image/png;base64,%s' % base64.b64encode(image.getvalue()).decode('ascii')
        return render_template('index.html', preview=image_url)


if __name__ == '__main__':
    app.run()
