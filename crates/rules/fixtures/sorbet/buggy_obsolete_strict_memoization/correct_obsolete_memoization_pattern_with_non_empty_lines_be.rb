def foo
  @foo = T.let(@foo, T.nilable(Foo))
 some_other_computation
  @foo ||= multiline_method_call(
    foo,
    bar,
    baz,
  )
end
