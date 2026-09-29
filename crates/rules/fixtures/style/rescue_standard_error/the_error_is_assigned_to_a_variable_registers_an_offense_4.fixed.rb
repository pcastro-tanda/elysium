def baz
  foo
rescue StandardError => e
  bar
end
