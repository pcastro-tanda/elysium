class Foo
  def self.inherited(base)
    do_something
    super
  end
end
