def foo
  raise ArgumentError, (<<~END.squish) unless guard
    A multiline message
    that will be squished.
  END

  return_value
end
