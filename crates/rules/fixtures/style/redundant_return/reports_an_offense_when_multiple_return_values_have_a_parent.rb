def do_something
  return (foo && bar), 42
  ^^^^^^ Redundant `return` detected. To return multiple values, use an array.
end
