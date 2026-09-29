if condition
  h[:key] = foo
  ^^^^^^^^^^^^^ Move `h[:key] = foo` out of the conditional.
  bar
else
  h[:key] = foo
  ^^^^^^^^^^^^^ Move `h[:key] = foo` out of the conditional.
  baz
end
