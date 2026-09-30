def foo
  raise ArgumentError, call(<<~END.squish) unless guard
    A multiline message
    that will be squished.
  END

  return_value
end
