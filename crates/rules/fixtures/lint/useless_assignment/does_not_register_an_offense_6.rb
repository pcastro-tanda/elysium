def testing
rescue Foo
  attempts += 1
  retry
rescue Bar
  attempts += 1
end
