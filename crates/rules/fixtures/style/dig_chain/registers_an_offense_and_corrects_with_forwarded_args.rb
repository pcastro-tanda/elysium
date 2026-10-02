def foo(...)
  x.dig(:foo).dig(...)
    ^^^^^^^^^^^^^^^^^^ Use `dig(:foo, ...)` instead of chaining.
end
