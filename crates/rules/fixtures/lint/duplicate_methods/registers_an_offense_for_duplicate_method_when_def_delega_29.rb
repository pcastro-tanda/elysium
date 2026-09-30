Class.new do
  def_delegators :foo, :bar, :baz

  def bar; end
  ^^^^^^^ Method `Object#bar` is defined at both (string):2 and (string):4.
end
