Class.new do
  def_delegator :foo, :bar, :baz

  def bar; end

  def baz; end
  ^^^^^^^ Method `Object#baz` is defined at both (string):2 and (string):6.
end
