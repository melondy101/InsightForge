import type { SVGProps } from 'react';
const base = (p: SVGProps<SVGSVGElement>) => ({
  viewBox:'0 0 24 24', fill:'none', stroke:'currentColor',
  strokeWidth:1.6, strokeLinecap:'round' as const, strokeLinejoin:'round' as const,
  'aria-hidden':true, ...p,
});
export const IconWorkspace=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><rect x="3" y="4" width="7" height="16" rx="1.5"/><rect x="14" y="4" width="7" height="10" rx="1.5"/></svg>);
export const IconDraft=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><path d="M7 3h7l4 4v14H7z"/><path d="M14 3v4h4"/></svg>);
export const IconReference=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><path d="M4 5h16M4 10h16M4 15h10"/><circle cx="18" cy="17" r="3"/></svg>);
export const IconThought=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><path d="M9 18h6M10 21h4"/><path d="M12 3a6 6 0 00-3.5 10.9c.6.5.9 1 .9 1.6V16h5.2v-.5c0-.6.3-1.1.9-1.6A6 6 0 0012 3z"/></svg>);
export const IconCoach=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><circle cx="12" cy="12" r="9"/><path d="M15.5 8.5l-2.1 5.4-5.4 2.1 2.1-5.4z"/></svg>);
export const IconSource=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><path d="M3 7l9-4 9 4-9 4-9-4z"/><path d="M3 7v10l9 4 9-4V7"/></svg>);
export const IconSearch=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><circle cx="11" cy="11" r="7"/><path d="M21 21l-4.3-4.3"/></svg>);
export const IconSettings=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.6 1.6 0 00.3 1.8 2 2 0 11-2.8 2.8 1.6 1.6 0 00-2.7 1.1 2 2 0 11-4 0A1.6 1.6 0 003 15a2 2 0 010-4 1.6 1.6 0 001.5-2.9 2 2 0 112.8-2.8A1.6 1.6 0 0010 4.7 2 2 0 1114 4a1.6 1.6 0 002.7 1.1 2 2 0 112.8 2.8A1.6 1.6 0 0021 11a2 2 0 010 4z"/></svg>);
export const IconEgress=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><path d="M12 3l7 3v5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6z"/></svg>);
export const IconSun=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M4.93 4.93l1.41 1.41m11.32 11.32 1.41 1.41M2 12h2m16 0h2M4.93 19.07l1.41-1.41m11.32-11.32 1.41-1.41"/></svg>);
export const IconMoon=(p:SVGProps<SVGSVGElement>)=>(<svg {...base(p)}><path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z"/></svg>);
