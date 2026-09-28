def m(foo)
  foo - 1 == 0
  ^^^^^^^^^^^^ Use `(foo - 1).zero?` instead of `foo - 1 == 0`.
end
