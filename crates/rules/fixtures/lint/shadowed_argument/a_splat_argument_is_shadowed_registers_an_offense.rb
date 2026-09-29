def do_something(*items)
  *items, last = [42, 42]
   ^^^^^ Argument `items` was shadowed by a local variable before it was used.
  puts items
end
