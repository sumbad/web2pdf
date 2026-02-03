async function main() {
  function cleanAngleBracketsForA11y(root = document.body) {
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const parent = node.parentElement;
        if (!parent) return NodeFilter.FILTER_REJECT;

        // Skip "code" blocks inside "pre"
        if (parent.closest("pre code")) {
          return NodeFilter.FILTER_REJECT;
        }

        if (/[<>]/.test(node.nodeValue)) {
          return NodeFilter.FILTER_ACCEPT;
        }

        return NodeFilter.FILTER_REJECT;
      },
    });

    const nodes = [];
    while (walker.nextNode()) {
      nodes.push(walker.currentNode);
    }

    for (const textNode of nodes) {
      textNode.nodeValue = textNode.nodeValue
        .replace(/</g, "‹")
        .replace(/>/g, "›");
    }
  }

  /**
   * Cleans text content within a node by removing unwanted characters
   * and normalizing whitespace. Also fixes code element spacing.
   */
  function cleanNodeText(node) {
    if (typeof node.innerHTML === 'function') {
      node.innerHTML = node.innerHTML
        .replace(/\s*<code([^>]*)>\s*/g, " <code$1>")
        .replace(/\s*<\/code>\s*/g, "</code> ");
    }

    if (
      node.nodeType === Node.TEXT_NODE &&
      !node.parentNode?.closest("pre code")
    ) {
      node.textContent = node.textContent
        .replace(/[\u200B-\u200D\uFEFF]/g, "") // zero-width characters
        .replace(/\u00A0/g, " ") // non-breaking space
        .replace(/\s+/g, " "); // normalize spaces
    } else if (node.nodeType === Node.ELEMENT_NODE) {
      node.childNodes.forEach(cleanNodeText);
    }
  }

  // Clean text in common content elements
  document
    .querySelectorAll("p, td, div, span, li, h1, h2, h3, h4, h5, h6")
    .forEach((el) => {
      cleanNodeText(el);
    });

  cleanAngleBracketsForA11y(document.body);

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
