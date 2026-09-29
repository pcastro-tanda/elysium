x = 1 && foo.bar

if true
  foo&.bar
elsif (foo.bar)
  call(1, 2, 3 + foo&.baz)
                    ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
else
  case
  when 1, foo&.bar
             ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
    [
      1,
      {
        2 => 3,
        foo&.baz => 4,
           ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
        4 => -foo&.zoo
                 ^^ Redundant safe navigation on non-nil receiver (detected by analyzing previous code/method invocations).
      }
    ]
  end
end
