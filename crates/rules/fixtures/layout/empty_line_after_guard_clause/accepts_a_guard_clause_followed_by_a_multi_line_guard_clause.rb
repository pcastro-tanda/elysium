def foo
  return if something?
  if something_else?
    raise bar(
      baz
    )
  end
end
