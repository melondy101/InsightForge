import { JSDOM } from 'jsdom';

const dom = new JSDOM('<!DOCTYPE html><html><body><div id="root"></div></body></html>');

Object.defineProperty(globalThis, 'document', {
  value: dom.window.document,
  writable: true,
});
Object.defineProperty(globalThis, 'window', {
  value: dom.window,
  writable: true,
});
Object.defineProperty(globalThis, 'navigator', {
  value: dom.window.navigator,
  writable: true,
});
Object.defineProperty(globalThis, 'HTMLElement', {
  value: dom.window.HTMLElement,
  writable: true,
});
