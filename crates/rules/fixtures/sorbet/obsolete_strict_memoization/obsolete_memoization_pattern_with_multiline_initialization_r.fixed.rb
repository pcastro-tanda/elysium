def foo
  @foo ||= T.let(
    multiline_method_call(
      foo,
      bar,
      baz,
    ),
    T.nilable(Foo),
  )
end
