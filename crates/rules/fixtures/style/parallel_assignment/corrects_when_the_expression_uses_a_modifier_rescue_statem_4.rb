def foo
  a, b = 1, 2 rescue foo
  ^^^^^^^^^^^ Do not use parallel assignment.
end
