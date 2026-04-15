import { JSDOM } from "jsdom";

if (typeof document === "undefined") {
  const dom = new JSDOM("<!doctype html><html><body></body></html>", {
    url: "http://localhost/",
  });

  globalThis.window = dom.window as typeof globalThis.window;
  globalThis.document = dom.window.document;
  globalThis.navigator = dom.window.navigator;
  globalThis.HTMLElement = dom.window.HTMLElement;
  globalThis.Node = dom.window.Node;
  globalThis.MutationObserver = dom.window.MutationObserver;
  globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window);
}

const htmlElementPrototype = globalThis.HTMLElement?.prototype as
  | (HTMLElement & {
      attachEvent?: (eventName: string, listener: EventListenerOrEventListenerObject) => void;
      detachEvent?: (eventName: string, listener: EventListenerOrEventListenerObject) => void;
    })
  | undefined;

if (htmlElementPrototype && typeof htmlElementPrototype.attachEvent !== "function") {
  // React can still take its legacy input-event path under Bun/jsdom. Mirror the
  // old IE hooks with no-op adapters so controlled inputs stay testable.
  htmlElementPrototype.attachEvent = () => {};
}

if (htmlElementPrototype && typeof htmlElementPrototype.detachEvent !== "function") {
  htmlElementPrototype.detachEvent = () => {};
}
