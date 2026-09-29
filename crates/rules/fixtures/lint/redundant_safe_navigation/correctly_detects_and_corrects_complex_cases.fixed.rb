x = 1 && foo.bar

if true
  foo&.bar
elsif (foo.bar)
  call(1, 2, 3 + foo.baz)
else
  case
  when 1, foo.bar
    [
      1,
      {
        2 => 3,
        foo.baz => 4,
        4 => -foo.zoo
      }
    ]
  end
end
