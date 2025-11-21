/**
 * Sets the document language for accessibility and PDF generation.
 * Defaults to 'en' if no language is already specified.
 */
function langSet() {
  document.documentElement.lang = document.documentElement.lang || "en";
  return document.documentElement.lang;
}

