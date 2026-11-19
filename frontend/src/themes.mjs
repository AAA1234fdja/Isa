// The three original palettes keep their existing CSS. New palettes are derived
// from hue and saturation so light and dark modes share a coordinated identity.
const original = [
  {id:'forest', name:'Forest', description:'Deep woodland greens', color:'#315f49', preview:'#d9ede0'},
  {id:'moss', name:'Moss', description:'Warm olive and leaf', color:'#687a35', preview:'#e7ebcf'},
  {id:'sage', name:'Sage', description:'Cool eucalyptus greens', color:'#318779', preview:'#d2ebe5'},
];
const additional = [
  ['ocean','Ocean','Deep sea blue',205,55],
  ['atlantic','Atlantic','Cool maritime blue',196,52],
  ['cobalt','Cobalt','Vivid mineral blue',221,62],
  ['indigo','Indigo','Rich blue violet',235,49],
  ['twilight','Twilight','Muted evening blue',246,33],
  ['lavender','Lavender','Soft violet tones',270,38],
  ['iris','Iris','Deep purple petals',280,48],
  ['plum','Plum','Dusky purple',300,38],
  ['berry','Berry','Deep berry pink',320,48],
  ['rose','Rose','Muted rose',343,40],
  ['cherry','Cherry','Deep crimson',354,58],
  ['ember','Ember','Warm glowing red',8,60],
  ['terracotta','Terracotta','Earthy fired clay',16,50],
  ['copper','Copper','Burnished orange',25,55],
  ['amber','Amber','Golden amber',36,60],
  ['saffron','Saffron','Warm golden yellow',45,56],
  ['honey','Honey','Mellow golden brown',42,42],
  ['sand','Sand','Soft desert neutrals',38,28],
  ['clay','Clay','Warm earthy brown',20,28],
  ['olive','Olive','Earthy olive green',75,39],
  ['fern','Fern','Fresh fern green',105,38],
  ['pine','Pine','Deep evergreen',145,35],
  ['emerald','Emerald','Rich emerald green',155,55],
  ['jade','Jade','Balanced jade green',165,39],
  ['mint','Mint','Cool mint green',170,28],
  ['teal','Teal','Deep blue green',180,49],
  ['lagoon','Lagoon','Clear tropical blue',188,55],
  ['slate','Slate','Cool blue gray',210,20],
  ['graphite','Graphite','Subtle charcoal',225,10],
  ['midnight','Midnight','Deep midnight blue',225,34],
].map(([id,name,description,hue,saturation])=>({
  id,name,description,hue,saturation,
  color:`hsl(${hue}, ${saturation}%, 28%)`,
  preview:`hsl(${hue}, ${saturation}%, 91%)`,
}));
export const themes = Object.freeze([...original, ...additional].map(theme=>Object.freeze(theme)));
export function paletteFor(theme, dark) {
  if (theme.hue === undefined) return null;
  const {hue, saturation} = theme;
  const values = dark ? [10,14,18,27,38,89] : [93,99,96,86,80,22];
  return Object.fromEntries(['side','main','card','active','border','ink'].map((name,index)=>[
    `--palette-${name}`, `hsl(${hue}, ${saturation}%, ${values[index]}%)`,
  ]));
}
