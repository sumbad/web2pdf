async function SanitationStyles() {
  const style = document.createElement("style");
  style.innerHTML = `
        p > code {
            display: inline !important;
            position: static !important;
            float: none !important;
            opacity: 1 !important;
            transform: none !important;

            filter: none !important;
            backdrop-filter: none !important;

            border-radius: none !important;;
            padding: unset !important;;
        }
    `;
  document.head.appendChild(style);

  return true;
}
