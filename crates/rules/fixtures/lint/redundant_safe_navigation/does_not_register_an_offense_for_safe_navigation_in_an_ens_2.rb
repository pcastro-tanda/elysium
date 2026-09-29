begin
  foo.bar
rescue
  handle
ensure
  foo&.baz
end
