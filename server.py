import os

from flask import Flask, request, make_response, render_template, jsonify

import svg2polylines


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


@app.route('/', methods=['GET'])
def hello():
    return render_template('index.html')


@app.route('/process', methods=['POST'])
def process():
    svg = request.data.strip()
    if not svg:
        return make_response('Empty request data', 400)

    polylines = svg2polylines.parse(svg)
    return jsonify(polylines)


if __name__ == '__main__':
    app.run()
