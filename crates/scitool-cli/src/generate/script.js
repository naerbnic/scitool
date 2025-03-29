'use strict';
(() => {
    for (const element of document.querySelectorAll('.copy-button')) {
        element.addEventListener('click', (event) => {
            const textToCopy = element.dataset.copytext;

            // text to clipboard
            navigator.clipboard.writeText(textToCopy).then(() => {
                console.log('Copied to clipboard: %s', textToCopy);

                // make a box for popup with transition fadeout to show user they've copied bit of script
                const dialog = document.createElement('div');
                dialog.textContent = `Copied: ${textToCopy}`;
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

                // make dialog box near cursor
                const mouseX = event.pageX;
                const mouseY = event.pageY;
                dialog.style.left = `${mouseX + 10}px`;
                dialog.style.top = `${mouseY + 10}px`;

                // make box go away after 2sec
                setTimeout(() => {
                    dialog.style.opacity = '0';
                    setTimeout(() => dialog.remove(), 500);
                }, 2000);
            });
        });
    }
})();
