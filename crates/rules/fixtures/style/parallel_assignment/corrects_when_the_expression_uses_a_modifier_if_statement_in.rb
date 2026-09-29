def foo
  a, b = 1, 2 if foo
  ^^^^^^^^^^^ Do not use parallel assignment.
end
