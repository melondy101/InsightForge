import { EditorView } from '@codemirror/view';
export const foundryTheme = EditorView.theme({
  '&':{backgroundColor:'transparent',color:'var(--text)',fontFamily:'var(--serif)',fontSize:'16.5px',lineHeight:'1.9'},
  '.cm-content':{caretColor:'var(--ember)',padding:'24px 0'},
  '.cm-cursor,.cm-dropCursor':{borderLeftColor:'var(--ember)',borderLeftWidth:'2px'},
  '&.cm-focused .cm-selectionBackground,.cm-selectionBackground':{background:'var(--ember-glow)'},
  '.cm-activeLine':{backgroundColor:'color-mix(in srgb, var(--ember) 7%, transparent)'},
  '.cm-gutters':{backgroundColor:'transparent',border:'none'},
},{dark:true});
