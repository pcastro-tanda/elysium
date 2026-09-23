foo = %r{
  foo # this should not affect the position of the escape below
  \-
  ^^ Redundant escape inside regexp literal
}x
