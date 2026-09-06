// Returns the length of meaningful text inside #content, or -1 if missing.
// Polled from Rust until the SPA renders the article (skeletons give ~0).
function main() {
  const content = document.querySelector("#content");

  if (!content) {
    return -1;
  }

  return content.textContent.trim().length;
}
