class << self
  def bar; end
  def baz; end

  module_function :bar, :baz
  ^^^^^^^^^^^^^^^ `module_function` should not be inlined in method definitions.
end
