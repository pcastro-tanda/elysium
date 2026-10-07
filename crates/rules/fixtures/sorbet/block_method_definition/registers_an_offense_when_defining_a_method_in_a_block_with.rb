yielding_method do
  puts _1

  def bad_method(args)
  ^^^^^^^^^^^^^^^^^^^^ Sorbet/BlockMethodDefinition: Do not define methods in blocks (use `define_method` as a workaround).
  end
end
