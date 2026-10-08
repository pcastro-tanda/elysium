class MyEnum < T::Enum
  prepend Comparable
  ^^^^^^^^^^^^^^^^^^ Sorbet/ForbidComparableTEnum: Do not use `T::Enum` as a comparable object because of significant performance overhead.
end
