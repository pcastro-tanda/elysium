yielding_method do
  def bad_method(arg0, arg1 = 1, *args, foo:, bar: nil, **kwargs, &block)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/BlockMethodDefinition: Do not define methods in blocks (use `define_method` as a workaround).
    if arg0
      arg0 + arg1
    end
  end
end
