import type { CSSProperties } from "react";

const paths = {
  reset: <><path d="M4 4v5h5M4.5 9a7 7 0 1 1-.2 5" /></>,
  palette: <><path d="M12 3a9 9 0 1 0 0 18h1a2 2 0 0 0 1-4c-1-1 0-3 2-3h2a3 3 0 0 0 3-3 9 9 0 0 0-9-8Z"/><path d="M7 9h.01M11 6h.01M16 7h.01M6 13h.01"/></>,
  temperature: <><path d="M9 14V5a3 3 0 0 1 6 0v9a5 5 0 1 1-6 0ZM12 6v11" /><circle cx="12" cy="18" r="1" /></>,
  thinking: <><path d="M9 16h7l4-5-4-6H9l-4 6 4 5ZM12 16v5M8 21h8" /><path d="m9 10 3 2 3-2" /></>,
  effort: <><circle cx="12" cy="12" r="9" /><path d="m8 16 2-6 6-2-2 6zM12 3v2M21 12h-2M12 21v-2M3 12h2" /></>,
  grammar: <><path d="m4 18 5-12 5 12M6 14h6M16 8h5M18.5 8v10" /></>,
  document: <><path d="M6 3h9l4 4v14H6zM14 3v5h5M9 12h7M9 16h5" /></>,
  settings: <><path d="M4 7h16M4 17h16" /><circle cx="9" cy="7" r="3" /><circle cx="15" cy="17" r="3" /></>,
  server: <><rect x="4" y="4" width="16" height="6" rx="2" /><rect x="4" y="14" width="16" height="6" rx="2" /><path d="M8 7h.01M8 17h.01M14 7h3M14 17h3" /></>,
  globe: <><circle cx="12" cy="12" r="9" /><ellipse cx="12" cy="12" rx="4" ry="9" /><path d="M3 12h18" /></>,
  key: <><circle cx="8" cy="8" r="4" /><path d="m11 11 9 9M16 16l3-3M18 18l3-3" /></>,
  model: <><rect x="6" y="6" width="12" height="12" rx="3" /><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4M10 10h4v4h-4z" /></>,
  code: <><path d="m8 7-5 5 5 5m8-10 5 5-5 5m-3-14-2 18" /></>,
  shield: <><path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6zM8 12l3 3 5-6" /></>,
  play: <path d="m8 4 12 8-12 8z" />,
  save: <><path d="M4 3h13l3 3v15H4zM8 3v6h8V3M8 21v-8h8v8" /></>,
  trash: <><path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7" /></>,
  sample: <><rect x="4" y="3" width="16" height="18" rx="2" /><path d="m8 9 2 2 4-4M8 15h8M8 18h5" /></>,
  check: <path d="m5 12 4 4L19 6" />,
  chevron: <path d="m9 5 7 7-7 7" />,
  info: <><circle cx="12" cy="12" r="9" /><path d="M12 11v6M12 7h.01" /></>,
  clock: <><circle cx="12" cy="12" r="9" /><path d="M12 7v5l3 2" /></>,
  link: <><path d="m10 13 4-4M8 16l-1 1a4 4 0 0 1-6-6l4-4a4 4 0 0 1 6 0m2 1 1-1a4 4 0 0 1 6 6l-4 4a4 4 0 0 1-6 0" transform="translate(1 0)" /></>,
};
export type IconName = keyof typeof paths;
export default function Icon({ name, size = 18, className = "", style }: { name: IconName; size?: number; className?: string; style?: CSSProperties }) {
  return <svg aria-hidden="true" className={`icon ${className}`} width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={style}>{paths[name]}</svg>;
}
