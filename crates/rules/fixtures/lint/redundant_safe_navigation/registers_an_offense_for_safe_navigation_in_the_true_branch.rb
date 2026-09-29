if foo&.bar&.baz
  foo&.qux
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
