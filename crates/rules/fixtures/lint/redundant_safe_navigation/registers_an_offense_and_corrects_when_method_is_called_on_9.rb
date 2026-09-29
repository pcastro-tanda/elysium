case
when 1
  2
when foo.bar
  foo&.bar
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
else
  foo&.baz
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
