const IBB_WIDTH = 358;
const IBB_HEIGHT = 123;
const PREVIEW_SCALE_FACTOR = 2; // Preview is scaled with a factor of 2

/**
 * Load an SVG file.
 */
function loadSvg(ev, svg, canvas) {
    if (svg.text) {
        const request = new XMLHttpRequest();
        request.open('POST', '/preview/', true);
        request.setRequestHeader('Content-Type', 'application/json');
        request.onload = function() {
            if (this.status == 200) {
                // Success
                const polylines = JSON.parse(this.response);
                canvas.clear();
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
    // Create group of all polylines
    const group = [];
    for (let polyline of polylines) {
        const polylineObj = new fabric.Polyline(
            preparePolyline(polyline, PREVIEW_SCALE_FACTOR),
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

    // Re-scale group to fit and center it in viewport
    const height = IBB_HEIGHT * PREVIEW_SCALE_FACTOR;
    const width = IBB_WIDTH * PREVIEW_SCALE_FACTOR;
    if ((groupObj.height / groupObj.width) > (height / width)) {
        groupObj.scaleToHeight(height);
    } else {
        groupObj.scaleToWidth(width);
    }
    const centerpoint = new fabric.Point(width / 2, height / 2);
    groupObj.setPositionByOrigin(centerpoint, 'center', 'center');

    // Add to canvas
    canvas.add(groupObj);
}

/**
 * Send the object to the printer.
 */
function printObject(svg, canvas) {
    return function(clickEvent) {
        const printMode = document.querySelector('input[name=mode]:checked').value;

        if (canvas.getObjects().length == 0) {
            alert('No object loaded. Please choose an SVG file first.');
            return;
        }

        canvas.forEachObject((obj, i) => {
            console.debug('Object', i + ':');
            const dx = (obj.left - obj._originalLeft) / PREVIEW_SCALE_FACTOR;
            const dy = (obj.top - obj._originalTop) / PREVIEW_SCALE_FACTOR;
            console.debug('  Moved by', dx, dy);
            console.debug('  Scaled by', obj.scaleX, obj.scaleY);

            const request = new XMLHttpRequest();
            request.open('POST', '/print/', true);
            request.setRequestHeader('Content-Type', 'application/json');
            request.onload = function() {
                if (this.status == 204) {
                    // Success TODO
                    if (printMode == 'once') {
                        alert('Printing!');
                    } else {
                        alert('Scheduled printing!');
                    }
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
                'offset_x': dx,
                'offset_y': dy,
                'scale_x': obj.scaleX,
                'scale_y': obj.scaleY,
                'mode': printMode,
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
