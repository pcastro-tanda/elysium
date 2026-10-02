class User < Foo
  def do_something
    all.where(id: ids)
  end
end
