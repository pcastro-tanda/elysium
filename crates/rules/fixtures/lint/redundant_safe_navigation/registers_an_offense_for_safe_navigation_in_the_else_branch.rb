begin
  foo.bar
rescue
  handle
else
  foo&.baz
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
