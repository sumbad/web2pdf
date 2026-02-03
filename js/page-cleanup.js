/**
 * Cleans the page content for PDF generation and screen readers.
 * Removes ads, cookies, footers, and cleans up text formatting.
 * Adds CSS styles to prevent text breaking in PDF output.
 */
function pageCleanup() {
  // Remove unwanted elements
  document
    .querySelectorAll(".ads, .cookie, .footer, footer")
    .forEach((e) => e.remove());

  /**
   * Cleans text content within a node by removing unwanted characters
   * and normalizing whitespace. Also fixes code element spacing.
   */
  function cleanNodeText(node) {
    if (node.tagName === "P") {
      node.innerHTML = node.innerHTML
        .replace(/\s*<code([^>]*)>\s*/g, "<code$1> ")
        .replace(/\s*<\/code>\s*/g, " </code>");
    }

    if (
      node.nodeType === Node.TEXT_NODE &&
      node.parentNode?.tagName !== "CODE"
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

  // Remove empty paragraphs
  document.querySelectorAll("p").forEach((p) => {
    if (!p.textContent.trim()) {
      p.remove();
    }
  });

  // Add CSS to prevent text breaking in PDF
  const style = document.createElement("style");
  style.innerHTML = `
        html, body, main {
            font-variation-settings: "wght" 400;
            letter-spacing: +0.01em !important;
            background: none;
        }
        * {
            font-family: Arial, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif !important;
            font-feature-settings: "kern" 0, "liga" 0, "calt" 0;
            font-kerning: none !important;
            font-variant-ligatures: none !important;
            -webkit-font-variant-ligatures: none !important;
            text-rendering: optimizeSpeed !important;
            -webkit-print-color-adjust: exact !important;
            image-rendering: optimizeQuality;
            color-adjust: exact;
        }
        p, li, td, th {
            page-break-inside: avoid;
            break-inside: avoid;
            orphans: 3;
            widows: 3;
            text-wrap: unset;
        }

        /* Safe lists for Chrome → PDF */
        ul, ol {
            list-style: initial !important;
            list-style-position: outside !important;
            padding-left: 1.6em !important;
        }
        li {
            display: list-item !important;
            width: auto !important;
            break-inside: avoid !important;
            text-indent: 0 !important;
            max-inline-size: none;
        }
    `;
  document.head.appendChild(style);

  return true;
}
