'use strict';
(() => {
    function showPopupMessage(bodyX, bodyY, msgText) {
        // make a box for popup with transition fadeout to show user they've copied bit of script
        const dialog = document.createElement('div');
        dialog.textContent = msgText;
        Object.assign(dialog.style, {
            position: 'absolute',
            backgroundColor: '#333',
            color: '#fff',
            padding: '8px 16px',
            borderRadius: '4px',
            fontSize: '14px',
            zIndex: '1000',
            transition: 'opacity 0.5s ease-out',
            opacity: '1',
            pointerEvents: 'none'
        });

        document.body.appendChild(dialog); // add box to html

        // make dialog box at position in body
        dialog.style.left = `${bodyX + 10}px`;
        dialog.style.top = `${bodyY + 10}px`;

        // make box go away after 2sec
        setTimeout(() => {
            dialog.style.opacity = '0';
            setTimeout(() => dialog.remove(), 500);
        }, 2000);
    }

    for (const element of document.querySelectorAll('.copy-button')) {
        element.addEventListener('click', (event) => {
            const textToCopy = element.dataset.copytext;

            // text to clipboard
            navigator.clipboard.writeText(textToCopy).then(() => {
                console.log('Copied to clipboard: %s', textToCopy);
                const mouseX = event.pageX;
                const mouseY = event.pageY;

                // make a box for popup with transition fadeout to show user they've copied bit of script
                showPopupMessage(mouseX, mouseY, `Copied ID To Clipboard: ${textToCopy}`);
            });
        });
    }

    for (const element of document.querySelectorAll('.link-button')) {
        element.addEventListener('click', (event) => {
            const linkId = element.dataset.linkid;

            // Construct a link to the current page, with the given fragment.
            let { origin, pathname } = new URL(window.location.href);
            const textToCopy = `${origin}${pathname}#${linkId}`;

            // text to clipboard
            navigator.clipboard.writeText(textToCopy).then(() => {
                console.log('Copied link to clipboard: %s', textToCopy);
                const mouseX = event.pageX;
                const mouseY = event.pageY;

                // make a box for popup with transition fadeout to show user they've copied bit of script
                showPopupMessage(mouseX, mouseY, `Copied Link To Clipboard: ${textToCopy}`);
            });
        });
    }
})();
