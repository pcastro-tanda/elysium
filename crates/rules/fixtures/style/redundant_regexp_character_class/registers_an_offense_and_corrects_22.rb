foo = /a#{/[b]/}c/
           ^^^ Redundant single-element character class, `[b]` can be replaced with `b`.
