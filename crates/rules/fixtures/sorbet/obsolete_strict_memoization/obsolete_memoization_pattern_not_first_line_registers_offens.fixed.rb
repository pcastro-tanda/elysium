def foo
  some
  other
  code
  @foo ||= T.let(Foo.new, T.nilable(Foo))
end
