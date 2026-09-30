Module.new do
  def_delegator :foo, :bar

  def bar; end
  ^^^^^^^ Method `Object#bar` is defined at both (string):2 and (string):4.
end
