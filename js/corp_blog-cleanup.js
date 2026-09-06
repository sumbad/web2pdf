function main() {
  document.querySelectorAll("*").forEach((el) => {
    const display = window.getComputedStyle(el).display;
    if (display === "grid" || display === "inline-grid") {
      el.style.display = "initial";
    }
  });

  const selectors = [
    "debugg",
    '[style*="mix-blend-mode"]',
    "#sticky-sidebar",
    "#header",
    "#primary-content > a",
    ".report-scroll__root-container > a"
  ];

  const removed = [];
  for (const selector of selectors) {
    const nodes = document.querySelectorAll(selector);
    if (nodes.length === 0) continue;

    nodes.forEach((el) => el.remove());
    removed.push(`${selector}:${nodes.length}`);
  }

  return removed.join(",") || "none";
}
