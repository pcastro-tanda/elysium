if foo&.bar
  foo&.bar.baz
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
