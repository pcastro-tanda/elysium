def foo
  @foo = ::T.let(@foo, ::T.nilable(Foo))
  @foo ||= Foo.new
end
