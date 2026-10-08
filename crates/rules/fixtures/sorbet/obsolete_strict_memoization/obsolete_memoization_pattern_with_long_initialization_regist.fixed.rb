def foo
  @foo ||= T.let(
    some_really_long_initialization_expression______________________________________,
    T.nilable(SomeReallyLongTypeName______________________________________),
  )
end
