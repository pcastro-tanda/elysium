def foo
  some
  other
  code
  @foo = T.let(@foo, T.nilable(Foo))
  @foo ||= Foo.new
end
