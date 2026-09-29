define_method(:values) do
  return @values if defined?(@values)
  @values = do_something
end
