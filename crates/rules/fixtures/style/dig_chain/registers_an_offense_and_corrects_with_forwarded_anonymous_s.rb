def foo(*)
  x.dig(*).dig(*)
    ^^^^^^^^^^^^^ Use `dig(*, *)` instead of chaining.
end
