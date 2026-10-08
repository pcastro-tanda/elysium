def foo
  @foo = T.let(@foo, T.nilable(Foo))

  @foo ||= multiline_method_call(
    foo,
    bar,
    baz,
  )
end
