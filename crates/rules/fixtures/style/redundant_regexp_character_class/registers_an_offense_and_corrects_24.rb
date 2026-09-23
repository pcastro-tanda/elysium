foo = /
  a # This comment shouldn't affect the position of the offense
  [b]
  ^^^ Redundant single-element character class, `[b]` can be replaced with `b`.
/x
