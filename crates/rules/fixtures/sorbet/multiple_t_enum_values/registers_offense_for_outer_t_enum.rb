class MyEnum < T::Enum
^^^^^^^^^^^^^^^^^^^^^^ Sorbet/MultipleTEnumValues: `T::Enum` should have at least two values.
  class NestedEnum < T::Enum
    enums do
      A = new
      B = new
    end
  end

  enums do
    C = new
  end
end
