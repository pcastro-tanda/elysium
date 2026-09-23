foo = %r{
  \/a
  ^^ Redundant escape inside regexp literal
  b\/
   ^^ Redundant escape inside regexp literal
}x
