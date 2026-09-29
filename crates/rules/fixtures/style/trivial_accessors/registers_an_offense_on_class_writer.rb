class Foo
  def self.foo=(val)
  ^^^ Use `attr_writer` to define trivial writer methods.
    @foo = val
  end
end
