async function main() {
  console.log("[HABR] preprocessing start");

  /************************************************************
   * Hide header, footer, sidebars and ads
   ************************************************************/
  const hideSelectors = [
    ".tm-header",
    ".tm-footer",
    ".tm-base-layout__header",
    ".tm-base-layout__footer",
    ".tm-layout__aside",
    ".tm-sidebar-wrapper",
    ".tm-layout__sidebar",
    ".tm-footer-menu",
    ".footer-menu",
    ".tm-notice",
    ".tm-page-wrapper__ads-container",
    ".tm-adapter-loader",
    ".tm-user-menu",
    ".tm-site-menu",
    ".header-banner-wrapper",
    ".digest-subscription",
    ".sponsor-block",
    ".tm-project-block--vacancies",
    ".tm-article-blocks__comments",
    ".tm-page__sidebar",
    ".tm-page-article__banner",
    ".tm-events-block",
  ];

  hideSelectors.forEach((sel) => {
    document.querySelectorAll(sel).forEach((el) => {
      el.style.display = "none";
    });
  });

  /************************************************************
   * Remove gray background and visual separators
   ************************************************************/
  document.body.style.background = "white";
  document.documentElement.style.background = "white";

  document.querySelectorAll(".tm-page__wrapper, .tm-layout").forEach((el) => {
    el.style.background = "white";
  });

  // Remove borders and separators that produce visual frames
  document.querySelectorAll("hr, .tm-divider, .tm-separated").forEach((el) => {
    el.style.display = "none";
  });

  /************************************************************
   * Remove registration prompts
   ************************************************************/
  document
    .querySelectorAll(".tm-block, .tm-article-body__register")
    .forEach((el) => {
      if (/зарегистрируйтесь|войдите/i.test(el.innerText || "")) {
        el.style.display = "none";
      }
    });

  /************************************************************
   * Remove recommendations and feed blocks
   ************************************************************/
  const killRecommends = [
    ".tm-article-blocks__comments + *", // remove all blocks after comments
    ".tm-other-articles",
    ".tm-articles-list",
    ".tm-turbo-wrapper",
    "section#more-like-this",
    "section.tm-feed",
    ".tm-feed",
  ];

  killRecommends.forEach((sel) => {
    document.querySelectorAll(sel).forEach((el) => {
      el.style.display = "none";
    });
  });

  /************************************************************
   * Normalize positioning for PDF
   ************************************************************/
  document.querySelectorAll("*").forEach((el) => {
    const st = getComputedStyle(el);
    if (st.position === "sticky" || st.position === "fixed") {
      el.style.position = "static";
    }
  });

  /************************************************************
   * Wait for images (article + comments)
   ************************************************************/
  // const imgs = Array.from(document.images).filter(
  //   (img) => img.complete === false,
  // );
  // console.log("[HABR] waiting for images:", imgs.length);
  //
  // await Promise.all(imgs.map((img) => img.decode().catch(() => {})));

  console.log("[HABR] preprocessing complete");
}
