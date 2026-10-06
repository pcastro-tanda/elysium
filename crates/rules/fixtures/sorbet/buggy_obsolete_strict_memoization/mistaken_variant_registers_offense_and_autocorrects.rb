def foo
  @foo = T.let(nil, T.nilable(Foo))
               ^^^ Sorbet/BuggyObsoleteStrictMemoization: This might be a mistaken variant of the two-stage workaround that used to be needed for memoization in `#typed: strict` files. See https://sorbet.org/docs/type-assertions#put-type-assertions-behind-memoization.
  @foo ||= Foo.new
end
