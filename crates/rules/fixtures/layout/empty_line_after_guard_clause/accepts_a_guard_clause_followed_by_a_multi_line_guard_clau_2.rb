def foo
  return if something?
  if something_else?
    fail bar(
      baz
    )
  end
end
