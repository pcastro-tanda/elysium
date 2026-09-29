if foo.condition?
  foo&.bar
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
