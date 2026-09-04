export type Theme='dark'|'light';
const KEY='if.theme';
export function getTheme():Theme{
  const s=localStorage.getItem(KEY);
  return s==='light'||s==='dark'?s:'dark';
}
export function applyTheme(t:Theme){document.documentElement.setAttribute('data-theme',t);}
export function setTheme(t:Theme){localStorage.setItem(KEY,t);applyTheme(t);}
