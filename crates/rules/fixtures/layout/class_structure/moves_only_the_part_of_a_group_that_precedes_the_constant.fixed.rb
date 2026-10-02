class Foo
  FIRST = 1
  def do_something; end
  DYNAMIC = do_something.freeze
  SECOND = 2
end
