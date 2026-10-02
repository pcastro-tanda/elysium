let :foo do \
            ^ Redundant line continuation.
  foo(bar, \
           ^ Redundant line continuation.
      baz)
end
