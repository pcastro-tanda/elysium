if foo.condition? && other_condition
  foo&.bar
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
