def foo
  return if something?
  if something_else?
    return bar(
      baz
    )
  end
end
