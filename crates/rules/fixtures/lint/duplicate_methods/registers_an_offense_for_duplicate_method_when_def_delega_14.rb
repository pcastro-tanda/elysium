A = Class.new do
  def_delegators :foo, :bar, :baz

  def bar; end
  ^^^^^^^ Method `A#bar` is defined at both (string):2 and (string):4.
end
