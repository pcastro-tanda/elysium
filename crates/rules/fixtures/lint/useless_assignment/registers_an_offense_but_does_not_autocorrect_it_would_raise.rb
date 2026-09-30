def foo
  bar &&= 1
  ^^^ Useless assignment to variable - `bar`. Use `&&` instead of `&&=`.
end
