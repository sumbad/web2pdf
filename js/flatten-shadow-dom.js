function flattenShadowDom() {
  const walker = document.createTreeWalker(document, NodeFilter.SHOW_ELEMENT);

  const hosts = [];
  while (walker.nextNode()) {
    const el = walker.currentNode;
    if (el.shadowRoot) hosts.push(el);
  }

  for (const el of hosts) {
    const id =
      el.getAttribute("data-flatten-id") ||
      "f" + Math.random().toString(36).slice(2);

    const newElem = document.createElement(`${el.tagName.toLowerCase()}-flat`);

    newElem.setAttribute("data-flatten-id", id);

    const shadow = el.shadowRoot;

    const frag = document.createDocumentFragment();

    // переносим элементы shadow root
    shadow.childNodes.forEach((node) => {
      // переносим <style>
      if (node.tagName === "STYLE") {
        const newStyle = document.createElement("style");
        let css = node.textContent;

        // корректная замена :host
        css = css.replace(/:host\b/g, `[data-flatten-id="${id}"]`);

        newStyle.textContent = css;
        frag.appendChild(newStyle);
      } else {
        // переносим svg / span / другие узлы
        frag.appendChild(node.cloneNode(true));
      }
    });

    // добавляем наружу
    newElem.appendChild(frag);
    el.insertAdjacentElement("afterend", newElem);
    el.remove();
  }

  return;
}
