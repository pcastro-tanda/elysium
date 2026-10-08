class Builder
  BASIC = T.let([:a, :b].freeze, T::Array[Symbol])
          ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/RedundantTLetForLiteral: Redundant `T.let` for Array literal. Sorbet can infer this type automatically.

  def build
    acc = BASIC
    [1, 2].each { |n| acc += [n.to_s.to_sym] }
    acc
  end
end
