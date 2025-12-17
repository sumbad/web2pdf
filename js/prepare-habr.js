async function prepareHabr() {
  console.log("[HABR] preprocessing start");

  /************************************************************
   * 1. Hide header, footer, sidebars and ads
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
  ];

  hideSelectors.forEach((sel) => {
    document.querySelectorAll(sel).forEach((el) => {
      el.style.display = "none";
    });
  });

  /************************************************************
   * 2. Remove gray background and visual separators
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
   * 3. Force comments section to render
   ************************************************************/
  const commentsAnchor = document.querySelector('a[href="#comments"]');
  if (commentsAnchor) {
    commentsAnchor.click();
  }

  // Scroll page to bottom to trigger IntersectionObserver
  for (let i = 0; i < 8; i++) {
    window.scrollBy(0, window.innerHeight);
    await new Promise((r) => setTimeout(r, 250));
  }

  // Force layout recalculation
  window.dispatchEvent(new Event("scroll"));
  window.dispatchEvent(new Event("resize"));

  /************************************************************
   * 4. Expand all comment threads
   ************************************************************/
  function clickAllButtons(selector) {
    let changed = true;
    let rounds = 0;

    while (changed && rounds < 20) {
      rounds++;
      changed = false;

      document.querySelectorAll(selector).forEach((btn) => {
        if (!btn.dataset._clicked && btn.offsetParent !== null) {
          btn.dataset._clicked = "1";
          btn.click();
          changed = true;
        }
      });
    }
  }

  clickAllButtons(".tm-comments__expand-button");
  clickAllButtons(".tm-comments__more-button");
  clickAllButtons("button[data-test-id='comment-toolbar-toggle-answers']");
  clickAllButtons("button[data-test-id='load-more-comments']");

  /************************************************************
   * 5. Remove registration prompts
   ************************************************************/
  document
    .querySelectorAll(".tm-block, .tm-article-body__register")
    .forEach((el) => {
      if (/зарегистрируйтесь|войдите/i.test(el.innerText || "")) {
        el.style.display = "none";
      }
    });

  /************************************************************
   * 6. Remove recommendations and feed blocks
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
   * 7. Normalize positioning for PDF
   ************************************************************/
  document.querySelectorAll("*").forEach((el) => {
    const st = getComputedStyle(el);
    if (st.position === "sticky" || st.position === "fixed") {
      el.style.position = "static";
    }
  });

  /************************************************************
   * 8. Wait for images (article + comments)
   ************************************************************/
  // const imgs = Array.from(document.images).filter(
  //   (img) => img.complete === false,
  // );
  // console.log("[HABR] waiting for images:", imgs.length);
  //
  // await Promise.all(imgs.map((img) => img.decode().catch(() => {})));

  console.log("[HABR] preprocessing complete");
}
