export function initializeSwipeNavigation() {
  const toggle = document.querySelector(".mobile-menu-toggle");
  const drawer = document.querySelector(".mobile-drawer");
  const backdrop = document.querySelector(".mobile-drawer-backdrop");
  if (!(toggle instanceof HTMLButtonElement) ||
      !(drawer instanceof HTMLElement) ||
      !(backdrop instanceof HTMLElement)) {
    return;
  }

  const closeButton = drawer.querySelector(".mobile-drawer-close");
  const closeTargets = document.querySelectorAll("[data-drawer-close]");
  const mobileViewport = window.matchMedia("(max-width: 720px)");
  const swipeThreshold = 56;
  let touchStart = null;

  function setOpen(open) {
    drawer.classList.toggle("is-open", open);
    backdrop.classList.toggle("is-open", open);
    drawer.setAttribute("aria-hidden", String(!open));
    drawer.inert = !open;
    toggle.setAttribute("aria-expanded", String(open));
    toggle.setAttribute("aria-label", open ? "Fermer le menu" : "Ouvrir le menu");
    document.body.classList.toggle("mobile-menu-open", open);
  }

  requestAnimationFrame(() => {
    drawer.classList.add("is-ready");
    backdrop.classList.add("is-ready");
  });

  toggle.addEventListener("click", () => {
    if (mobileViewport.matches) {
      setOpen(toggle.getAttribute("aria-expanded") !== "true");
    }
  });
  for (const target of closeTargets) {
    target.addEventListener("click", () => setOpen(false));
  }
  closeButton?.addEventListener("click", () => setOpen(false));

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && toggle.getAttribute("aria-expanded") === "true") {
      setOpen(false);
      toggle.focus();
    }
  });

  document.addEventListener("touchstart", (event) => {
    if (!mobileViewport.matches || event.touches.length !== 1) {
      touchStart = null;
      return;
    }
    const touch = event.touches[0];
    touchStart = {
      x: touch.clientX,
      y: touch.clientY,
      wasOpen: toggle.getAttribute("aria-expanded") === "true"
    };
  }, { passive: true });

  document.addEventListener("touchend", (event) => {
    if (!touchStart || event.changedTouches.length !== 1) {
      touchStart = null;
      return;
    }
    const touch = event.changedTouches[0];
    const deltaX = touch.clientX - touchStart.x;
    const deltaY = touch.clientY - touchStart.y;
    const startedAtLeftEdge = touchStart.x <= 28;
    if (Math.abs(deltaX) >= swipeThreshold &&
        Math.abs(deltaX) > Math.abs(deltaY) * 1.3) {
      if (deltaX > 0 && !touchStart.wasOpen && startedAtLeftEdge) {
        setOpen(true);
      } else if (deltaX < 0 && touchStart.wasOpen) {
        setOpen(false);
      }
    }
    touchStart = null;
  }, { passive: true });

  mobileViewport.addEventListener("change", (event) => {
    if (!event.matches) {
      setOpen(false);
    }
  });
}
