case foo.condition
when 1
  foo&.bar
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
when foo&.baz
        ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
  2
else
  foo&.zoo
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
end
