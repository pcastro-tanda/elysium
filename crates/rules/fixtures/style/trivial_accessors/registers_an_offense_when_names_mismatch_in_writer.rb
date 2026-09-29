class Foo
  def foo=(val)
  ^^^ Use `attr_writer` to define trivial writer methods.
    @f = val
  end
end
