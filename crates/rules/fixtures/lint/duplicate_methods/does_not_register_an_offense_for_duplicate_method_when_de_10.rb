A.class_eval do
  def_delegators :foo, :bar, :baz if qux?

  def bar; end
end
