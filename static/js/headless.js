/**
 * Load the list of SVG files.
 */
function loadSvgList() {
    const request = new XMLHttpRequest();
    request.open('GET', '/list/', true);
    request.setRequestHeader('Content-Type', 'application/json');
    request.onload = function() {
        if (this.status == 200) {
            // Success
            const files = JSON.parse(this.response);
            console.log('Loaded list of SVG files');
            const element = document.querySelector('#svgfiles');
            // Hide loading text
            element.querySelector('.loading').hidden = true;
            // Show files
            if (files.length === 0) {
                element.querySelector('.nofiles').hidden = false;
            } else {
                const list = element.querySelector('ul.files');
                for (const file of files) {
                    const entry = document.createElement('li');
                    entry.appendChild(document.createTextNode(file));
                    list.appendChild(entry);
                }
                list.hidden = false;
            }
        } else {
            // Error
            console.error('Error: HTTP', this.status);
            const element = document.querySelector('#svgfiles');
            // Hide loading text
            element.querySelector('.loading').hidden = true;
            // Show error
            const error = element.querySelector('.error');
            error.innerText = 'Error fetching SVG files (HTTP ' + this.status + ')';
            error.hidden = false;
        }
    }
    request.send();
}

/**
 * Load the configuration.
 */
function loadConfig() {
    const request = new XMLHttpRequest();
    request.open('GET', '/config/', true);
    request.setRequestHeader('Content-Type', 'application/json');
    request.onload = function() {
        if (this.status == 200) {
            // Success
            const config = JSON.parse(this.response);
            console.log('Loaded config');
            const element = document.querySelector('#config');
            // Hide loading text
            element.querySelector('.loading').hidden = true;
            // Show config
            const items = element.querySelector('dl.items');
            const configEntries = [
                {key: "device", label: "Device"},
                {key: "svg_dir", label: "SVG Directory"},
            ];
            for (const item of configEntries) {
                const key = document.createElement('dt');
                key.appendChild(document.createTextNode(item.label));
                items.appendChild(key);
                const value = document.createElement('dd');
                const valueCode = document.createElement('code');
                valueCode.appendChild(document.createTextNode(config[item.key]));
                value.appendChild(valueCode)
                items.appendChild(value);
            }
            items.hidden = false;
        } else {
            // Error
            console.error('Error: HTTP', this.status);
            const element = document.querySelector('#config');
            // Hide loading text
            element.querySelector('.loading').hidden = true;
            // Show error
            const error = element.querySelector('.error');
            error.innerText = 'Error fetching config (HTTP ' + this.status + ')';
            error.hidden = false;
        }
    }
    request.send();
}

ready(() => {
    console.info('Started.');

    loadConfig();
    loadSvgList();
});
