A = Class.new do
  def_delegator :foo, :bar if baz?

  def bar; end
end
