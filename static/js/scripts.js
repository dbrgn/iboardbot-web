function ready(fn) {
    if (document.readyState != 'loading'){
        fn();
    } else {
        document.addEventListener('DOMContentLoaded', fn);
    }
}

function loadSvg(ev, svg, canvas) {
    if (svg.text) {
        const request = new XMLHttpRequest();
        request.open('POST', '/preview/', true);
        request.setRequestHeader('Content-Type', 'application/json');
        request.onload = function() {
            if (this.status == 200) {
                // Success
                const polylines = JSON.parse(this.response);
                drawPreview(canvas, polylines);
            } else {
                // Error
                console.error('Error: HTTP', this.status);
                if (this.status == 400) {
                    alert('Error. Did you upload a valid SVG file?');
                } else {
                    alert('Error (HTTP ' + this.status + ')');
                }
            }
        }
        request.send(JSON.stringify({svg: svg.text}));
    }
}

/**
 * Scale and transform polyline so it can be used by fabric.js.
 */
function preparePolyline(polyline, scaleFactor) {
    return polyline.map((pair) => ({
        x: pair.x * scaleFactor,
        y: pair.y * scaleFactor,
    }));
}

function drawPreview(canvas, polylines) {
    const group = [];
    for (let polyline of polylines) {
        const polylineObj = new fabric.Polyline(
            preparePolyline(polyline, 2),
            {
                stroke: 'black',
                fill: null,
                lockUniScaling: true,
                lockRotation: true,
            }
        );
        group.push(polylineObj);
    }
    const groupObj = new fabric.Group(group);
    canvas.add(groupObj);
}

function printObject(svg, canvas) {
    return function(clickEvent) {
        canvas.forEachObject((obj, i) => {
            console.debug('Object', i + ':');
            const dx = obj.left - obj._originalLeft;
            const dy = obj.top - obj._originalTop;
            console.debug('Moved by', dx, dy);
            console.debug('Scaled by', obj.scaleX, obj.scaleY);

            const request = new XMLHttpRequest();
            request.open('POST', '/print/', true);
            request.setRequestHeader('Content-Type', 'application/json');
            request.onload = function() {
                if (this.status == 200) {
                    // Success TODO
                } else {
                    // Error
                    console.error('Error: HTTP', this.status);
                    if (this.status == 400) {
                        alert('Error. Did you upload a valid SVG file?');
                    } else {
                        alert('Error (HTTP ' + this.status + ')');
                    }
                }
            }
            request.send(JSON.stringify({
                'svg': svg.text,
                'offsetX': dx,
                'offsetY': dy,
                'scaleX': obj.scaleX,
                'scaleY': obj.scaleY,
            }));
        });
    }
}

ready(() => {
    console.info('Started.');

    // Fabric.js canvas object
    const canvas = new fabric.Canvas('preview');
    let svg = {
        text: '',
    }

    const fileInput = document.querySelector('input[name=file]');
    fileInput.addEventListener('change', (changeEvent) => {
        const file = fileInput.files[0];
        if (file !== undefined) {
            const fr = new FileReader();
            fr.onload = function(ev) {
                svg.text = ev.target.result;
                loadSvg.bind(this)(ev, svg, canvas);
            }
            fr.readAsText(file);
        }
    });

    const print = document.querySelector('input#print');
    print.addEventListener('click', printObject(svg, canvas));
});
