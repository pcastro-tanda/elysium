def foo
  @foo = T.let(@foo, T.nilable(SomeReallyLongTypeName______________________________________))
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/ObsoleteStrictMemoization: This two-stage workaround for memoization in `#typed: strict` files is no longer necessary. See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization.
  @foo ||= some_really_long_initialization_expression______________________________________
end
