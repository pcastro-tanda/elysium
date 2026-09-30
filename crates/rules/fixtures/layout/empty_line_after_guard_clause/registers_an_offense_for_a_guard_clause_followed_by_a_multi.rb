def foo
  return if something?
  ^^^^^^^^^^^^^^^^^^^^ Add empty line after guard clause.
  if something_else?
    bar(
      baz
    )
  end
end
