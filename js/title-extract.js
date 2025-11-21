/**
 * Extracts the page title for PDF generation.
 * Returns the document title or falls back to the current URL.
 */
function titleExtract() {
  return document.title || window.location.href;
}

