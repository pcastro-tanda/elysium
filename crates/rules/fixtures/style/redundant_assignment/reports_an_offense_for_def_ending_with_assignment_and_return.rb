def func
  some_preceding_statements
  x = something
  ^^^^^^^^^^^^^ Redundant assignment before returning detected.
  x
end
