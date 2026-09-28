class Foo
end
foo = Foo.new
def foo.bar
  def baz
  ^^^^^^^ Method definitions must not be nested. Use `lambda` instead.
  end
end
