Foo.configure do |c|
  def bar
    return if baz?
  end
end
