class MyEnum < T::Enum
  include Comparable
  ^^^^^^^^^^^^^^^^^^ Sorbet/ForbidComparableTEnum: Do not use `T::Enum` as a comparable object because of significant performance overhead.
end
