def foo
  @foo ||= T.let(Foo.new, T.nilable(Foo))
end
