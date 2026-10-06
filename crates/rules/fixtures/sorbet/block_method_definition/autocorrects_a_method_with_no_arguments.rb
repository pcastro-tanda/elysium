yielding_method do
  def bad_method
  ^^^^^^^^^^^^^^ Sorbet/BlockMethodDefinition: Do not define methods in blocks (use `define_method` as a workaround).
    if arg0
      arg0 + arg1
    end
  end
end
