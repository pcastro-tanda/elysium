foo.each do
  it.bar
  it&.baz
    ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
