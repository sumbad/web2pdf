function updateIconifyIcon() {
  document.querySelectorAll("iconify-icon").forEach((el) => {
    const newEl = el.cloneNode();
    newEl.setAttribute("noobserver", "");

    el.insertAdjacentElement("afterend", newEl);
    el.remove();
  });

  return;
}
