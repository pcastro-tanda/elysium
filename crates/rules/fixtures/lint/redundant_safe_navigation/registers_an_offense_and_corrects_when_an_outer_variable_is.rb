val = foo
val.bar
baz.each { val&.qux }
              ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
