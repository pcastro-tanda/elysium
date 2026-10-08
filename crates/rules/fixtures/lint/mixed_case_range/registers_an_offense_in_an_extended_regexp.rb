foo = /
  A-z # not a character class
  [A-z]_..._
   ^^^ Ranges from upper to lower case ASCII letters may include unintended characters. Instead of `A-z` (which also includes several symbols) specify each range individually: `A-Za-z` and individually specify any symbols.
/x
