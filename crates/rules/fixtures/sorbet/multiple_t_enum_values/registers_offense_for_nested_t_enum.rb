class MyEnum < T::Enum
  class NestedEnum < T::Enum
  ^^^^^^^^^^^^^^^^^^^^^^^^^^ Sorbet/MultipleTEnumValues: `T::Enum` should have at least two values.
    enums do
      A = new
    end
  end

  enums do
    B = new
    C = new
  end
end
