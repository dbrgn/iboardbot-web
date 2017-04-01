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

function scalePolyline(polyline, factor) {
    return polyline.map((pair) => [pair[0] * factor, pair[1] * factor]);
}

function drawPreview(polylines) {
    const canvas = document.getElementById('preview');
    const ctx = canvas.getContext('2d');
    for (let polyline of polylines) {
        const scaled = scalePolyline(polyline, 2);
        ctx.beginPath();
        ctx.strokeStyle = 'black';
        ctx.moveTo(scaled[0][0], scaled[0][1]);
        for (let pos of scaled) {
            ctx.lineTo(pos[0], pos[1]);
        }
        ctx.stroke();
    }
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
