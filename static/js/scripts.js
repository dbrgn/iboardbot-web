function ready(fn) {
    if (document.readyState != 'loading'){
        fn();
    } else {
        document.addEventListener('DOMContentLoaded', fn);
    }
}

function loadSvg(ev) {
    const svgText = ev.target.result;
    if (svgText) {
        const request = new XMLHttpRequest();
        request.open('POST', '/process', true);
        request.setRequestHeader('Content-Type', 'image/svg+xml');
        request.onload = function() {
            if (this.status == 200) {
                // Success
                const polylines = JSON.parse(this.response);
                drawPreview(polylines);
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
        request.send(svgText);
    }
}

/**
 * Scale and transform polyline so it can be used by fabric.js.
 */
function preparePolyline(polyline, scaleFactor) {
    return polyline.map((pair) => ({
        x: pair[0] * scaleFactor,
        y: pair[1] * scaleFactor,
    }));
}

function drawPreview(polylines) {
    const canvas = new fabric.Canvas('preview');
    const group = [];
    for (let polyline of polylines) {
        const polylineObj = new fabric.Polyline(
            preparePolyline(polyline, 2),
            {
                stroke: 'black',
                fill: null,
//                left: 0,
//                top: 0,
            }
        );
        group.push(polylineObj);
    }
    canvas.add(new fabric.Group(group));
}

ready(() => {
    console.info('Started.');

    const fileInput = document.querySelector('input[name=file]');
    fileInput.addEventListener('change', (changeEvent) => {
        const file = fileInput.files[0];
        if (file !== undefined) {
            const fr = new FileReader();
            fr.onload = loadSvg;
            fr.readAsText(file);
        }
    });
});
