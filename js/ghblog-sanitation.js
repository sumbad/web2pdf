async function main() {
  /**
   * Cleans text content within a node by removing unwanted characters
   * and normalizing whitespace. Also fixes code element spacing.
   */
  function cleanNodeText(node) {
    if (typeof node.innerHTML === "string") {
      node.innerHTML = node.innerHTML
        .replace(/\s+<code([^>]*)>\s*/g, " <code$1>")
        .replace(/\s*<\/code>\s+/g, "</code> ")
        .replace(/\s+<a([^>]*)>\s*/g, " <a$1>")
        .replace(/\s*<\/a>\s+/g, "</a> ");
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

  document
    .querySelectorAll("p, td, div, span, li, h1, h2, h3, h4, h5, h6, a")
    .forEach((el) => {
      cleanNodeText(el);
    });

  const selectors = [
    "header#header",
    "footer",
    "#start-of-content > div.py-6.py-lg-8.color-bg-subtle",
    "#start-of-content > aside",
    "#start-of-content > section.container-xl.p-responsive-blog.mx-auto.mt-12.mb-8"
  ];

  for (const selector of selectors) {
    const nodes = document.querySelectorAll(selector);
    if (nodes.length === 0) continue;

    nodes.forEach((el) => el.remove());
  }

  return true;
}
