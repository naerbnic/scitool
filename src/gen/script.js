'use strict';
(() => {
    for (const element of document.querySelectorAll('.copy-button')) {
        element.addEventListener('click', (event) => {
            navigator.clipboard.writeText(element.dataset.copytext).then(() => {
                console.log('Copied to clipboard: %s', element.dataset.copytext);
            })
        });
    }
})();
