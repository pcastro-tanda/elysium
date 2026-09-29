def foo
  bar
ensure
  [1, 2, [3]]
  ^^^^^^^^^^^ Literal `[1, 2, [3]]` used in void context.
end
