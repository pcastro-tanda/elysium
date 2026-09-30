def foo
  raise ArgumentError, <<~END.squish.it.good unless guard
    A multiline message
    that will be squished.
  END

  return_value
end
