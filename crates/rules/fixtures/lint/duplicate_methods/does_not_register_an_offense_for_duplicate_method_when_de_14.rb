Module.new do
  def_delegators :foo, :bar, :baz if qux?

  def bar; end
end
