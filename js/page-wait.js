async function pageWait() {
  // DOM loaded
  if (document.readyState !== "complete") {
    await new Promise((r) => addEventListener("load", r, { once: true }));
  }

  // Force load lazy images
  document.querySelectorAll('img[loading="lazy"]').forEach((img) => {
    img.loading = "auto";
    img.src = img.src;
  });

  // Wait for images (safe)
  const imgs = Array.from(document.images);
  await Promise.all(
    imgs.map(
      (img) =>
        new Promise((resolve) => {
          if (img.complete && img.naturalWidth > 0) return resolve();

          const timer = setTimeout(resolve, 3000);

          img.addEventListener(
            "load",
            () => {
              clearTimeout(timer);
              resolve();
            },
            { once: true },
          );

          img.addEventListener(
            "error",
            () => {
              clearTimeout(timer);
              resolve();
            },
            { once: true },
          );
        }),
    ),
  );

  // Paint & composite
  await new Promise((resolve) =>
    requestAnimationFrame(() =>
      requestAnimationFrame(() => requestAnimationFrame(resolve)),
    ),
  );

  return true;
}
