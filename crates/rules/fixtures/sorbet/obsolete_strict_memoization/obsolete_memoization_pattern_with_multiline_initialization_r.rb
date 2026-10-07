def foo
  @foo = T.let(@foo, T.nilable(Foo))
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/ObsoleteStrictMemoization: This two-stage workaround for memoization in `#typed: strict` files is no longer necessary. See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization.
  @foo ||= multiline_method_call(
    foo,
    bar,
    baz,
  )
end
