{
  bar: foo.bar,
  baz: foo&.baz,
          ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
  foo&.zoo => 3
     ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
}
