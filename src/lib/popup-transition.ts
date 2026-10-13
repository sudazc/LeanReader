import { cubicOut } from "svelte/easing";
import { fade, fly, scale } from "svelte/transition";

function prefersReducedMotion() {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

export function fadeTransition(node: Element, options: { duration?: number } = {}) {
  const duration = options.duration ?? 120;
  return fade(node, {
    duration: prefersReducedMotion() ? Math.min(duration, 100) : duration,
  });
}

export function panelTransition(
  node: Element,
  options: { duration?: number; x?: number } = {},
) {
  const duration = options.duration ?? 140;
  if (prefersReducedMotion()) {
    return fade(node, { duration: Math.min(duration, 100) });
  }

  return fly(node, {
    x: options.x ?? 8,
    duration,
    easing: cubicOut,
  });
}

export function popupTransition(
  node: Element,
  options: { duration?: number; start?: number } = {},
) {
  const duration = options.duration ?? 150;
  if (prefersReducedMotion()) {
    return fade(node, { duration: Math.min(duration, 100) });
  }

  return scale(node, {
    duration,
    easing: cubicOut,
    start: options.start ?? 0.97,
  });
}
